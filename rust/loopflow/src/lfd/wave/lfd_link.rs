//! Best-effort link from a running wave server to lfd.
//!
//! `lf wave <name>` is its own process; lfd neither launches nor supervises
//! it. This module closes the loop between the two, in both directions:
//!
//! - **Registration.** On boot the server registers itself as the wave's
//!   `WaveAgent` session (`POST /v0/waves/{wave}/agent/register`, source
//!   `wave_server`, endpoint + pid in the row) so Concerto's agent tree shows
//!   the mind and one-brain enforcement has a fact to key on. A live brain
//!   already registered is a refusal (clear error naming it) unless `--force`
//!   takes over. lfd unreachable → warn once, keep serving fully functional,
//!   retry lazily on [`LinkConfig::register_retry`]. Graceful shutdown
//!   deregisters (`POST /sessions/{id}/complete`, exit 0 — the session id is
//!   its own completion token); a crash is closed by lfd's session
//!   reconciliation (pid probe on `wave_server` rows).
//!
//! - **Observation.** A tail task subscribes to lfd's `/ws` event stream and
//!   journals confirmed worker facts for this wave: `WorkerDispatched` when a
//!   worker session+run appears, `WorkerFinished` when it goes terminal
//!   (summary = what lfd knows: session exit status, run error, PR url). The
//!   stream carries no worktree placement, so the journal doesn't fake one.
//!   These are OBSERVATIONS — the server is never in the dispatch path. On
//!   drop the tail reconnects with backoff, and on every (re)connect it
//!   reconciles a snapshot (`GET /v0/sessions`, `GET /v0/waves/{id}/runs`) so
//!   missed events don't lose facts; the runtime's run-id guard keeps every
//!   fact journaled exactly once.
//!
//! **The lfd-down story, honestly:** no daemon means no registration, no
//! one-brain enforcement (today's status quo — two brains only collide if you
//! also run the old loop, which needs lfd anyway), and no worker observations
//! (`lfq worker run` needs lfd too, so there are no workers to observe).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use crate::lfd::wave::journal::WorkerOutcome;
use crate::lfd::wave::runtime::WaveRuntime;

/// Default cadence for lazy registration retries while lfd is unreachable —
/// deliberately coarse (the wave is fully functional without lfd).
pub const REGISTER_RETRY: Duration = Duration::from_secs(60);

/// HTTP timeout for every lfd call: registration must never wedge boot, and
/// the observer's snapshot fetches must never wedge the tail.
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

/// Observer reconnect backoff bounds (exponential between them).
const RECONNECT_FLOOR: Duration = Duration::from_secs(1);
const RECONNECT_CEILING: Duration = Duration::from_secs(30);

/// Everything the link needs to reach lfd and identify this server.
#[derive(Debug, Clone)]
pub struct LinkConfig {
    /// lfd base URL, e.g. `http://127.0.0.1:2486`.
    pub base_url: String,
    /// Wave name (lfd resolves names as well as ids in wave paths).
    pub wave: String,
    /// This server's loopback `host:port` (the `.wave-endpoint` address).
    pub endpoint: String,
    /// The repo root the server runs in.
    pub cwd: String,
    /// This server's pid, recorded for lfd's crash reconciliation.
    pub pid: u32,
    /// Take over from an existing live wave-agent session.
    pub force: bool,
    /// Lazy registration retry cadence (see [`REGISTER_RETRY`]).
    pub register_retry: Duration,
    /// Observer reconnect floor (tests shrink it; production uses
    /// [`RECONNECT_FLOOR`]).
    pub reconnect_floor: Duration,
}

impl LinkConfig {
    /// Production config: lfd address/token resolution shared with the rest
    /// of the CLI (`LFD_URL`/`LFD_HTTP_ADDR` env, local default).
    pub fn resolve(wave: &str, cwd: &str, force: bool) -> Self {
        Self {
            base_url: crate::lfd::client::resolve_base_url(),
            wave: wave.to_string(),
            endpoint: String::new(), // filled in once the listener is bound
            cwd: cwd.to_string(),
            pid: std::process::id(),
            force,
            register_retry: REGISTER_RETRY,
            reconnect_floor: RECONNECT_FLOOR,
        }
    }
}

/// What lfd handed back for a successful registration.
#[derive(Debug, Clone)]
pub struct Registration {
    pub session_id: String,
    pub wave_id: String,
}

/// The registration slot shared between the link task (writes on success)
/// and the server's shutdown path (takes it to deregister exactly once).
pub type SharedRegistration = Arc<Mutex<Option<Registration>>>;

/// One registration attempt's outcome.
#[derive(Debug)]
pub enum RegisterAttempt {
    Registered(Registration),
    /// Another live wave-agent session exists (409). The message names it.
    Refused {
        message: String,
    },
    /// lfd unreachable or unable to register (connection error, 5xx, wave
    /// not in lfd yet) — soft failure, retry lazily.
    Unreachable(String),
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .expect("reqwest client with a static config always builds")
}

fn authorize(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match crate::lfd::client::auth_header() {
        Some(token) => request.bearer_auth(token),
        None => request,
    }
}

#[derive(Debug, Deserialize)]
struct RegisteredSessionBody {
    id: String,
    wave_id: String,
}

/// One registration round-trip. Never panics, never blocks past
/// [`HTTP_TIMEOUT`].
pub async fn register_once(config: &LinkConfig) -> RegisterAttempt {
    let url = format!(
        "{}/v0/waves/{}/agent/register",
        config.base_url, config.wave
    );
    let body = serde_json::json!({
        "endpoint": config.endpoint,
        "pid": config.pid,
        "cwd": config.cwd,
        "force": config.force,
    });
    let response = match authorize(http_client().post(url)).json(&body).send().await {
        Ok(response) => response,
        Err(err) => return RegisterAttempt::Unreachable(err.to_string()),
    };
    let status = response.status();
    if status == reqwest::StatusCode::CONFLICT {
        let message = response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|value| {
                value["error"]["message"]
                    .as_str()
                    .map(|message| message.to_string())
            })
            .unwrap_or_else(|| "another wave-agent session is live".to_string());
        return RegisterAttempt::Refused { message };
    }
    if !status.is_success() {
        return RegisterAttempt::Unreachable(format!("lfd returned {status}"));
    }
    match response.json::<RegisteredSessionBody>().await {
        Ok(session) => RegisterAttempt::Registered(Registration {
            session_id: session.id,
            wave_id: session.wave_id,
        }),
        Err(err) => RegisterAttempt::Unreachable(format!("bad register response: {err}")),
    }
}

/// Deregister on graceful shutdown, blocking and bounded — callable from the
/// Ctrl-C interrupt hook (a plain thread) and via `spawn_blocking` from the
/// async shutdown path. Best-effort: a failure just leaves the row for lfd's
/// reconciliation.
pub fn deregister_blocking(config: &LinkConfig, registration: &Registration) {
    let url = format!(
        "{}/v0/sessions/{}/complete",
        config.base_url, registration.session_id
    );
    let Ok(client) = crate::lfd::client::blocking_client(HTTP_TIMEOUT) else {
        return;
    };
    let request = client
        .post(url)
        .header(
            crate::lfd::http::routes::session_controls::COMPLETION_TOKEN_HEADER,
            &registration.session_id,
        )
        .json(&serde_json::json!({ "exit_code": 0 }));
    let request = crate::lfd::client::authorize(request);
    if let Err(err) = request.send() {
        tracing::warn!(error = %err, "wave server deregistration failed; lfd reconciliation will close the session");
    }
}

/// The long-lived link task: ensure registered (lazy retry while lfd is
/// down), then observe lfd's event stream until the server shuts down. The
/// boot-time registration, if it succeeded, is already in `slot`.
pub async fn run_link(runtime: Arc<WaveRuntime>, config: LinkConfig, slot: SharedRegistration) {
    let registration = loop {
        if let Some(registration) = slot.lock().expect("registration slot poisoned").clone() {
            break registration;
        }
        tokio::time::sleep(config.register_retry).await;
        match register_once(&config).await {
            RegisterAttempt::Registered(registration) => {
                tracing::info!(
                    wave = config.wave,
                    session_id = registration.session_id,
                    "registered with lfd as the wave's agent session"
                );
                *slot.lock().expect("registration slot poisoned") = Some(registration.clone());
                break registration;
            }
            RegisterAttempt::Refused { message } => {
                // Another brain appeared while lfd was down. Don't take over
                // silently — keep retrying; it may deregister.
                tracing::warn!(wave = config.wave, message, "lfd registration refused");
            }
            RegisterAttempt::Unreachable(err) => {
                tracing::debug!(wave = config.wave, error = err, "lfd still unreachable");
            }
        }
    };
    observe(runtime, &config, &registration.wave_id).await;
}

// -- Observation tail ---------------------------------------------------

/// The subset of a run this observer cares about, from `RunDto` JSON
/// (unknown fields ignored).
#[derive(Debug, Clone, Deserialize)]
struct RunInfo {
    id: String,
    flow: String,
    task: Option<String>,
    error: Option<String>,
    pr: Option<PrInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct PrInfo {
    url: String,
}

/// The subset of a session this observer cares about — same keys on the WS
/// event payload (`Event::SessionCreated/Updated.session`) and the
/// `GET /v0/sessions` DTO.
#[derive(Debug, Clone, Deserialize)]
struct SessionInfo {
    id: String,
    wave_id: String,
    #[serde(rename = "use")]
    session_use: String,
    run_id: Option<String>,
    status: String,
}

#[derive(Debug, Deserialize)]
struct ListBody<T> {
    data: Vec<T>,
}

struct Observer {
    runtime: Arc<WaveRuntime>,
    config: LinkConfig,
    wave_id: String,
    http: reqwest::Client,
    /// Runs fetched from lfd, refreshed on cache miss and before every
    /// finish summary (the PR lands late in a run's life).
    runs: HashMap<String, RunInfo>,
}

/// Tail lfd's event stream forever: connect, reconcile a snapshot, stream;
/// on drop, reconnect with backoff and reconcile again. Runs until the task
/// is aborted at server shutdown.
async fn observe(runtime: Arc<WaveRuntime>, config: &LinkConfig, wave_id: &str) {
    let mut observer = Observer {
        runtime,
        config: config.clone(),
        wave_id: wave_id.to_string(),
        http: http_client(),
        runs: HashMap::new(),
    };
    let mut backoff = config.reconnect_floor;
    loop {
        match connect_ws(config).await {
            Ok(mut stream) => {
                backoff = config.reconnect_floor;
                // Snapshot AFTER subscribing: an event that races the
                // snapshot replays after it and the run-id guard dedupes,
                // so nothing falls between them.
                if let Err(err) = observer.reconcile_snapshot().await {
                    tracing::warn!(error = %err, "wave observer snapshot reconcile failed");
                }
                while let Some(message) = stream.next().await {
                    match message {
                        Ok(Message::Text(text)) => observer.on_ws_text(&text).await,
                        Ok(Message::Close(_)) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                tracing::debug!("lfd event stream closed; reconnecting");
            }
            Err(err) => {
                tracing::debug!(error = %err, "lfd event stream connect failed");
            }
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(RECONNECT_CEILING);
    }
}

async fn connect_ws(
    config: &LinkConfig,
) -> anyhow::Result<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
> {
    let ws_base = config
        .base_url
        .replacen("http://", "ws://", 1)
        .replacen("https://", "wss://", 1);
    let mut request = format!("{ws_base}/ws").into_client_request()?;
    if let Some(token) = crate::lfd::client::auth_header() {
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {token}").parse()?);
    }
    let (stream, _) = tokio_tungstenite::connect_async(request).await?;
    Ok(stream)
}

impl Observer {
    async fn get_json<T: serde::de::DeserializeOwned>(&self, path: &str) -> anyhow::Result<T> {
        let url = format!("{}{path}", self.config.base_url);
        let response = authorize(self.http.get(url)).send().await?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("lfd returned {status} for {path}");
        }
        Ok(response.json().await?)
    }

    async fn refresh_runs(&mut self) -> anyhow::Result<()> {
        let body: ListBody<RunInfo> = self
            .get_json(&format!("/v0/waves/{}/runs", self.wave_id))
            .await?;
        self.runs = body
            .data
            .into_iter()
            .map(|run| (run.id.clone(), run))
            .collect();
        Ok(())
    }

    async fn run_info(&mut self, run_id: &str, refresh: bool) -> Option<RunInfo> {
        if refresh || !self.runs.contains_key(run_id) {
            if let Err(err) = self.refresh_runs().await {
                tracing::warn!(run_id, error = %err, "failed to fetch wave runs from lfd");
            }
        }
        self.runs.get(run_id).cloned()
    }

    /// Reconcile a full snapshot of this wave's worker sessions — journal
    /// only deltas vs what the journal already knows.
    async fn reconcile_snapshot(&mut self) -> anyhow::Result<()> {
        let body: ListBody<SessionInfo> = self
            .get_json(&format!("/v0/sessions?wave_id={}", self.wave_id))
            .await?;
        for session in body.data {
            self.observe_session(&session).await;
        }
        Ok(())
    }

    async fn on_ws_text(&mut self, text: &str) {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
            return;
        };
        if !matches!(
            value["type"].as_str(),
            Some("session_created") | Some("session_updated")
        ) {
            return;
        }
        let Ok(session) = serde_json::from_value::<SessionInfo>(value["session"].clone()) else {
            return;
        };
        self.observe_session(&session).await;
    }

    /// Fold one observed session fact into the journal: a worker session
    /// with a run is a dispatch; a terminal one is a finish. Idempotent via
    /// the runtime's run-id guard — live events and snapshots overlap freely.
    async fn observe_session(&mut self, session: &SessionInfo) {
        if session.wave_id != self.wave_id || session.session_use != "worker" {
            return;
        }
        let Some(run_id) = session.run_id.clone() else {
            return;
        };
        if !self.runtime.worker_known(&run_id) {
            let Some(run) = self.run_info(&run_id, false).await else {
                tracing::warn!(
                    run_id,
                    "observed worker session with no fetchable run; skipped"
                );
                return;
            };
            if self.runtime.journal_worker_dispatched(
                &run_id,
                &session.id,
                &run.flow,
                run.task.as_deref().unwrap_or(""),
            ) {
                tracing::info!(
                    run_id,
                    session_id = session.id,
                    flow = run.flow,
                    "worker dispatched"
                );
            }
        }
        let terminal = matches!(session.status.as_str(), "succeeded" | "failed" | "canceled");
        if !terminal {
            return;
        }
        let outcome = if session.status == "succeeded" {
            WorkerOutcome::Completed
        } else {
            WorkerOutcome::Failed
        };
        // Refresh: the PR url and run error land late in a run's life.
        let run = self.run_info(&run_id, true).await;
        let mut summary = format!("session {}", session.status);
        if let Some(run) = run {
            if let Some(pr) = run.pr {
                summary.push_str(&format!("; pr {}", pr.url));
            }
            if let Some(error) = run.error {
                summary.push_str(&format!("; error: {error}"));
            }
        }
        if self
            .runtime
            .journal_worker_finished(&run_id, outcome, &summary)
        {
            tracing::info!(run_id, outcome = outcome.name(), summary, "worker finished");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    use axum::extract::ws::WebSocketUpgrade;
    use axum::extract::State;
    use axum::routing::{get, post};
    use axum::{Json, Router};

    use crate::lfd::wave::journal::{journal_path, EventKind, Journal};

    const WAVE_ID: &str = "wave-test-1";

    /// Scripted lfd stub. Stage advances with WS connections:
    /// - register: 500 on the first call (soft failure → lazy retry), then 200.
    /// - ws connection 1: emits `session_created` (worker running), then closes.
    /// - ws connection 2+: emits `session_updated` (worker succeeded), stays open.
    /// - sessions snapshot: run-1's worker always; from connection 2 also the
    ///   run-2 worker whose dispatch happened while the tail was down.
    /// - runs: both runs, run-1 with a PR.
    #[derive(Default)]
    struct StubState {
        register_calls: AtomicU32,
        ws_connections: AtomicU32,
    }

    fn worker_session(run: &str, status: &str) -> serde_json::Value {
        serde_json::json!({
            "id": format!("sess-{run}"),
            "wave_id": WAVE_ID,
            "use": "worker",
            "run_id": run,
            "status": status,
            "step": "dispatch:implement",
            "agent": "lf",
            "cwd": "/tmp/repo",
            "argv": [],
            "env": {},
            "source": "wave_step_tmux",
            "tmux_name": "lf-x",
            "created_at": "2026-07-04T00:00:00Z",
        })
    }

    async fn stub_lfd(state: Arc<StubState>) -> String {
        let register_state = state.clone();
        let sessions_state = state.clone();
        let ws_state = state.clone();
        let app = Router::new()
            .route(
                "/v0/waves/{wave}/agent/register",
                post(move |_state: State<()>| {
                    let state = register_state.clone();
                    async move {
                        if state.register_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                            return Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR);
                        }
                        Ok(Json(serde_json::json!({
                            "id": "sess-brain",
                            "object": "session",
                            "wave_id": WAVE_ID,
                        })))
                    }
                }),
            )
            .route(
                "/v0/sessions",
                get(move || {
                    let state = sessions_state.clone();
                    async move {
                        let mut data = vec![worker_session("run-1", "running")];
                        if state.ws_connections.load(Ordering::SeqCst) >= 2 {
                            data.push(worker_session("run-2", "running"));
                        }
                        Json(serde_json::json!({ "object": "list", "data": data, "has_more": false }))
                    }
                }),
            )
            .route(
                "/v0/waves/{wave}/runs",
                get(|| async {
                    Json(serde_json::json!({
                        "object": "list",
                        "data": [
                            {
                                "id": "run-1",
                                "flow": "implement",
                                "task": "wire the tail",
                                "status": "running",
                                "error": null,
                                "pr": { "url": "https://github.com/x/y/pull/7" },
                            },
                            {
                                "id": "run-2",
                                "flow": "design",
                                "task": "sketch the next item",
                                "status": "running",
                                "error": null,
                                "pr": null,
                            },
                        ],
                        "has_more": false,
                    }))
                }),
            )
            .route(
                "/ws",
                get(move |upgrade: WebSocketUpgrade| {
                    let state = ws_state.clone();
                    async move {
                        // Count BEFORE the 101 goes out: once the client's
                        // connect resolves, the snapshot endpoints already
                        // reflect this connection.
                        let connection = state.ws_connections.fetch_add(1, Ordering::SeqCst) + 1;
                        upgrade.on_upgrade(move |mut socket| async move {
                            if connection == 1 {
                                let event = serde_json::json!({
                                    "type": "session_created",
                                    "session": worker_session("run-1", "running"),
                                    "timestamp": "2026-07-04T00:00:00Z",
                                });
                                let _ = socket
                                    .send(axum::extract::ws::Message::Text(
                                        event.to_string().into(),
                                    ))
                                    .await;
                                // Give the client a beat to process, then drop
                                // the connection to force a reconnect.
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                return; // socket closes
                            }
                            let event = serde_json::json!({
                                "type": "session_updated",
                                "session": worker_session("run-1", "succeeded"),
                                "timestamp": "2026-07-04T00:00:01Z",
                            });
                            let _ = socket
                                .send(axum::extract::ws::Message::Text(
                                    event.to_string().into(),
                                ))
                                .await;
                            // Hold the stream open until the test ends.
                            while socket.recv().await.is_some() {}
                        })
                    }
                }),
            )
            .with_state(());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        format!("http://{addr}")
    }

    fn link_config(base_url: String) -> LinkConfig {
        LinkConfig {
            base_url,
            wave: "ship".to_string(),
            endpoint: "127.0.0.1:9".to_string(),
            cwd: "/tmp/repo".to_string(),
            pid: std::process::id(),
            force: false,
            register_retry: Duration::from_millis(25),
            reconnect_floor: Duration::from_millis(25),
        }
    }

    async fn wait_for(what: &str, cond: impl Fn() -> bool) {
        for _ in 0..400 {
            if cond() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("condition not met in time: {what}");
    }

    #[tokio::test]
    async fn register_once_is_soft_against_a_dead_lfd() {
        // Nothing listens here: registration must fail soft, fast.
        let config = link_config("http://127.0.0.1:1".to_string());
        match register_once(&config).await {
            RegisterAttempt::Unreachable(_) => {}
            other => panic!("expected Unreachable, got {other:?}"),
        }
    }

    /// The whole link, end to end, across a reconnect: registration retries
    /// lazily past a failed attempt; the first WS connection delivers a live
    /// dispatch; the drop + reconnect reconciles a snapshot that carries a
    /// missed dispatch and dedupes the already-journaled one; the live
    /// terminal event journals exactly one finish with lfd's facts.
    #[tokio::test]
    async fn link_journals_worker_facts_exactly_once_across_reconnect() {
        let stub_state = Arc::new(StubState::default());
        let base_url = stub_lfd(stub_state.clone()).await;
        let tmp = tempfile::tempdir().expect("tempdir");
        let (runtime, _inbox) =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");

        let slot: SharedRegistration = Arc::default();
        let link = tokio::spawn(run_link(
            runtime.clone(),
            link_config(base_url),
            slot.clone(),
        ));

        // Registration succeeded on a retry (first attempt got a 500).
        wait_for("registered", || slot.lock().unwrap().is_some()).await;
        assert!(
            stub_state.register_calls.load(Ordering::SeqCst) >= 2,
            "registration retried past the failed attempt"
        );

        // Across the scripted disconnect: both dispatches and the finish land.
        let rt = runtime.clone();
        wait_for("run-2 dispatched via snapshot", || rt.worker_known("run-2")).await;
        let rt = runtime.clone();
        wait_for("run-1 finished via live event", || {
            rt.in_flight_workers().iter().all(|w| w.run_id != "run-1") && rt.worker_known("run-1")
        })
        .await;
        link.abort();

        // Exactly once, each — the guard held across live + snapshot overlap.
        let (_, events) = Journal::open(&journal_path(tmp.path(), "ship")).expect("reopen");
        let dispatched: Vec<String> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::WorkerDispatched {
                    run_id, flow, task, ..
                } => Some(format!("{run_id}/{flow}/{task}")),
                _ => None,
            })
            .collect();
        let mut sorted = dispatched.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            vec![
                "run-1/implement/wire the tail".to_string(),
                "run-2/design/sketch the next item".to_string(),
            ]
        );
        let finished: Vec<(String, String)> = events
            .iter()
            .filter_map(|e| match &e.kind {
                EventKind::WorkerFinished {
                    run_id, summary, ..
                } => Some((run_id.clone(), summary.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].0, "run-1");
        assert!(
            finished[0].1.contains("succeeded")
                && finished[0].1.contains("https://github.com/x/y/pull/7"),
            "summary carries what lfd knows: {}",
            finished[0].1
        );
    }
}
