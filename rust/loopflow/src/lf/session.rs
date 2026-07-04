//! Self-registration of bare `lf` runs as lfd sessions.
//!
//! Managed sessions get `LFD_WAVE_ID`/`LFD_SESSION_ID` from the lfd executor,
//! and `lfq` chains wave/parent attribution off that env. A bare `lf design`
//! typed inside such a session used to be invisible — no Session row, no wave
//! attribution. This module closes the gap: when an agent-launching `lf`
//! command starts inside a wave context, it registers itself with lfd and
//! marks itself terminal when the run ends. No wave env or lfd unreachable →
//! exactly the old behavior, silently.
//!
//! # Env contract
//!
//! - `LFD_WAVE_ID` — wave attribution. Set by the lfd executor, inherited down
//!   the process tree. Absent → never register.
//! - `LFD_SESSION_ID` — the nearest enclosing session. The executor sets it on
//!   the process it launches, pointing at that process's *own* session row. A
//!   self-registered `lf` overwrites it for its descendants with the id of the
//!   session it just registered, so grandchildren chain parentage correctly.
//! - `LFD_SESSION_INHERITED` — whose session `LFD_SESSION_ID` is. The executor
//!   never sets it, so `LFD_SESSION_ID` without the marker means "this very
//!   process already has a session row" (executor-launched) — registering
//!   again would double-count the run. Every `lf` exports
//!   `LFD_SESSION_INHERITED=1` before spawning anything, flipping the meaning
//!   for descendants to "an ancestor's session — register your own row with it
//!   as parent".

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::lfd::client::{authorize, blocking_client, resolve_base_url};
use crate::lfd::http::dto::{RegisterSessionRequestDto, RegisterSessionResponseDto};
use crate::lfd::http::routes::session_controls::COMPLETION_TOKEN_HEADER;

pub const WAVE_ID_ENV: &str = "LFD_WAVE_ID";
pub const SESSION_ID_ENV: &str = "LFD_SESSION_ID";
pub const SESSION_INHERITED_ENV: &str = "LFD_SESSION_INHERITED";

const LFD_TIMEOUT: Duration = Duration::from_secs(3);

/// Where this `lf` invocation stands relative to lfd's session table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunContext {
    /// No wave env: not inside a wave context. Leave everything untouched.
    Outside,
    /// Executor-launched: lfd already created this process's session row and
    /// set `LFD_SESSION_ID` to it. Registering again would double-count.
    OwnSession,
    /// Inside a wave context without an own session row: register one.
    NeedsRegistration {
        wave_id: String,
        parent_session_id: Option<String>,
    },
}

/// The double-registration rule, in one place: `LFD_SESSION_ID` without
/// `LFD_SESSION_INHERITED` is *this process's* session (the executor made the
/// row); with the marker it is an ancestor's session and this run registers
/// its own row underneath it.
pub fn classify_run_context(
    wave_id: Option<&str>,
    session_id: Option<&str>,
    session_inherited: bool,
) -> RunContext {
    let Some(wave_id) = wave_id.filter(|value| !value.is_empty()) else {
        return RunContext::Outside;
    };
    let session_id = session_id.filter(|value| !value.is_empty());
    if session_id.is_some() && !session_inherited {
        return RunContext::OwnSession;
    }
    RunContext::NeedsRegistration {
        wave_id: wave_id.to_string(),
        parent_session_id: session_id.map(str::to_string),
    }
}

/// Register this `lf` invocation as a session if it runs inside a wave
/// context. Returns a guard to complete with the run's exit code; dropping it
/// unfinished (panic, early error) records a failure, and Ctrl+C reports via
/// the interrupt-hook machinery. Returns `None` — with zero noise — outside
/// wave contexts, for executor-launched runs, and when lfd is unreachable.
pub fn register_run(step: &str, agent: &str, argv: &[String]) -> Option<RunSession> {
    let context = classify_run_context(
        env_var(WAVE_ID_ENV).as_deref(),
        env_var(SESSION_ID_ENV).as_deref(),
        env_var(SESSION_INHERITED_ENV).is_some(),
    );
    let (wave_id, parent_session_id) = match context {
        RunContext::Outside => return None,
        RunContext::OwnSession => {
            mark_child_sessions_inherited();
            return None;
        }
        RunContext::NeedsRegistration {
            wave_id,
            parent_session_id,
        } => (wave_id, parent_session_id),
    };

    let request = RegisterSessionRequestDto {
        wave_id,
        parent_session_id,
        step: step.to_string(),
        agent: agent.to_string(),
        cwd: std::env::current_dir()
            .map(|cwd| cwd.display().to_string())
            .unwrap_or_default(),
        argv: argv.to_vec(),
        tmux_name: current_tmux_session_name(),
    };
    let session = register_session(&resolve_base_url(), &request).ok()?;

    // Descendants chain off the new session; the marker keeps reading as
    // "an ancestor's session" for them.
    std::env::set_var(SESSION_ID_ENV, session.inner.session_id.as_str());
    mark_child_sessions_inherited();

    let interrupted = Arc::clone(&session.inner);
    crate::engine::agent::register_interrupt_cleanup(move || interrupted.complete(130));
    Some(session)
}

/// Flip `LFD_SESSION_ID`'s meaning for everything this process spawns: the
/// row belongs to an ancestor, not to the spawned process. Called by every
/// `lf` command — including non-registering ones like `lf op`/`lf wave`,
/// which may themselves be the executor-launched session owner.
pub fn mark_child_sessions_inherited() {
    if env_var(SESSION_ID_ENV).is_some() {
        std::env::set_var(SESSION_INHERITED_ENV, "1");
    }
}

/// A session registered for this process. Complete it with the run's exit
/// code; dropping it unfinished records a failure.
#[derive(Debug)]
pub struct RunSession {
    inner: Arc<SessionHandle>,
}

impl RunSession {
    pub fn complete(&self, exit_code: i32) {
        self.inner.complete(exit_code);
    }

    pub fn session_id(&self) -> &str {
        &self.inner.session_id
    }
}

impl Drop for RunSession {
    fn drop(&mut self) {
        // Panic / early-error safety net: a run that never reported failed.
        self.inner.complete(1);
    }
}

#[derive(Debug)]
struct SessionHandle {
    base_url: String,
    session_id: String,
    completion_token: String,
    completed: AtomicBool,
}

impl SessionHandle {
    fn complete(&self, exit_code: i32) {
        if self.completed.swap(true, Ordering::SeqCst) {
            return;
        }
        let Ok(client) = blocking_client(LFD_TIMEOUT) else {
            return;
        };
        let url = format!("{}/v0/sessions/{}/complete", self.base_url, self.session_id);
        let _ = authorize(client.post(&url))
            .header(COMPLETION_TOKEN_HEADER, &self.completion_token)
            .json(&serde_json::json!({ "exit_code": exit_code }))
            .send();
    }
}

fn register_session(base_url: &str, request: &RegisterSessionRequestDto) -> Result<RunSession> {
    let client = blocking_client(LFD_TIMEOUT)?;
    let url = format!("{base_url}/v0/sessions/register");
    let response = authorize(client.post(&url)).json(request).send()?;
    if !response.status().is_success() {
        return Err(anyhow!("lfd returned {} for {url}", response.status()));
    }
    let registered: RegisterSessionResponseDto = response.json()?;
    Ok(RunSession {
        inner: Arc::new(SessionHandle {
            base_url: base_url.to_string(),
            session_id: registered.session.id,
            completion_token: registered.completion_token,
            completed: AtomicBool::new(false),
        }),
    })
}

fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|value| !value.is_empty())
}

fn current_tmux_session_name() -> Option<String> {
    std::env::var_os("TMUX")?;
    let output = std::process::Command::new("tmux")
        .args(["display-message", "-p", "#S"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!name.is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::sync::Mutex;

    use crate::lfd::http::dto::RegisterSessionRequestDto;

    use super::{classify_run_context, register_run, register_session, RunContext};

    // ── classify_run_context: the disambiguation rule ───────────────────

    #[test]
    fn no_wave_env_means_no_registration() {
        assert_eq!(
            classify_run_context(None, Some("sess-1"), true),
            RunContext::Outside
        );
        assert_eq!(
            classify_run_context(Some(""), None, false),
            RunContext::Outside
        );
    }

    #[test]
    fn executor_launched_runs_do_not_register_again() {
        // LFD_SESSION_ID without LFD_SESSION_INHERITED is this very process's
        // session row: the executor created it. Registering would double-count.
        assert_eq!(
            classify_run_context(Some("wave-1"), Some("sess-1"), false),
            RunContext::OwnSession
        );
    }

    #[test]
    fn inherited_session_id_becomes_the_parent() {
        assert_eq!(
            classify_run_context(Some("wave-1"), Some("sess-1"), true),
            RunContext::NeedsRegistration {
                wave_id: "wave-1".to_string(),
                parent_session_id: Some("sess-1".to_string()),
            }
        );
    }

    #[test]
    fn wave_context_without_session_registers_a_root_child() {
        assert_eq!(
            classify_run_context(Some("wave-1"), None, false),
            RunContext::NeedsRegistration {
                wave_id: "wave-1".to_string(),
                parent_session_id: None,
            }
        );
    }

    // ── register_run env behavior ────────────────────────────────────────

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    fn clear_session_env() {
        for key in [
            super::WAVE_ID_ENV,
            super::SESSION_ID_ENV,
            super::SESSION_INHERITED_ENV,
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    fn register_run_without_env_is_a_no_op() {
        let _guard = env_lock().lock().expect("env lock");
        clear_session_env();

        // Classification short-circuits before any HTTP client is built, so
        // this holds even with a live lfd on the machine.
        assert!(register_run("design", "lf", &["lf".to_string()]).is_none());
        assert!(std::env::var(super::SESSION_INHERITED_ENV).is_err());
    }

    #[test]
    fn executor_launched_run_marks_children_without_registering() {
        let _guard = env_lock().lock().expect("env lock");
        clear_session_env();
        std::env::set_var(super::WAVE_ID_ENV, "wave-1");
        std::env::set_var(super::SESSION_ID_ENV, "sess-1");

        let registration = register_run("design", "lf", &["lf".to_string()]);

        assert!(registration.is_none());
        // Children now read LFD_SESSION_ID as their parent's session.
        assert_eq!(
            std::env::var(super::SESSION_INHERITED_ENV).as_deref(),
            Ok("1")
        );
        // The executor's own value is untouched.
        assert_eq!(
            std::env::var(super::SESSION_ID_ENV).as_deref(),
            Ok("sess-1")
        );
        clear_session_env();
    }

    // ── HTTP round trip against a canned lfd ─────────────────────────────

    /// One-shot HTTP server: captures a single request (start line + body)
    /// and answers with the given JSON body.
    fn spawn_canned_lfd(response_json: String) -> (String, mpsc::Receiver<(String, String)>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock lfd");
        let addr = listener.local_addr().expect("mock lfd addr");
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            let mut start_line = String::new();
            reader.read_line(&mut start_line).expect("read start line");
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).expect("read header");
                let line = line.trim();
                if line.is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    content_length = value.trim().parse().expect("content length");
                }
            }
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body).expect("read body");
            sender
                .send((
                    start_line.trim().to_string(),
                    String::from_utf8(body).expect("utf8 body"),
                ))
                .ok();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_json.len(),
                response_json
            );
            reader
                .into_inner()
                .write_all(response.as_bytes())
                .expect("write response");
        });
        (format!("http://{addr}"), receiver)
    }

    fn session_json(id: &str) -> String {
        format!(
            r#"{{"session":{{"id":"{id}","object":"session","wave_id":"wave-1",
                "run_id":null,"parent_session_id":"sess-1","use":"worker",
                "step":"design","agent":"lf","cwd":"/tmp/repo",
                "argv":["lf","design"],"env":{{}},"source":"lf_cli",
                "tmux_name":"","status":"running","created_at":"2026-07-04T00:00:00Z",
                "attached_at":null,"started_at":null,"completed_at":null}},
                "completion_token":"{id}"}}"#
        )
    }

    #[test]
    fn register_session_posts_wave_parent_and_argv() {
        let (base_url, requests) = spawn_canned_lfd(session_json("sess-2"));

        let request = RegisterSessionRequestDto {
            wave_id: "wave-1".to_string(),
            parent_session_id: Some("sess-1".to_string()),
            step: "design".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: vec!["lf".to_string(), "design".to_string()],
            tmux_name: None,
        };
        let session = register_session(&base_url, &request).expect("register should succeed");
        assert_eq!(session.session_id(), "sess-2");

        let (start_line, body) = requests.recv().expect("mock lfd saw the request");
        assert!(start_line.starts_with("POST /v0/sessions/register"));
        let sent: RegisterSessionRequestDto =
            serde_json::from_str(&body).expect("request body round-trips");
        assert_eq!(sent, request);

        // Consumed without completing: the drop safety net reports failure,
        // but the mock only serves one connection — just don't panic.
        drop(session);
    }

    #[test]
    fn complete_posts_exit_code_with_token_once() {
        let (base_url, requests) = spawn_canned_lfd(session_json("sess-3"));
        let request = RegisterSessionRequestDto {
            wave_id: "wave-1".to_string(),
            parent_session_id: None,
            step: "design".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: vec!["lf".to_string()],
            tmux_name: None,
        };
        let session = register_session(&base_url, &request).expect("register should succeed");
        requests.recv().expect("registration request");

        let (complete_url, complete_requests) = spawn_canned_lfd("{}".to_string());
        // Redirect completion to a fresh one-shot server.
        let session = super::RunSession {
            inner: std::sync::Arc::new(super::SessionHandle {
                base_url: complete_url,
                session_id: session.session_id().to_string(),
                completion_token: "sess-3".to_string(),
                completed: std::sync::atomic::AtomicBool::new(false),
            }),
        };
        session.complete(7);
        drop(session); // second complete must not fire

        let (start_line, body) = complete_requests.recv().expect("completion request");
        assert!(start_line.starts_with("POST /v0/sessions/sess-3/complete"));
        let sent: serde_json::Value = serde_json::from_str(&body).expect("json body");
        assert_eq!(sent["exit_code"], 7);
        assert!(
            complete_requests.recv().is_err(),
            "drop after complete must not send a second request"
        );
    }
}
