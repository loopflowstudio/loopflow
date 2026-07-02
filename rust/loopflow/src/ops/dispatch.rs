use std::time::Duration;

use crate::lfd::client::{authorize, blocking_client, resolve_base_url};
use crate::lfd::http::dto::WaveRunDto;
use crate::ops::error::{OpsError, OpsResult};

const DISPATCH_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone)]
pub struct DispatchOptions {
    pub wave: String,
    pub flow: String,
    pub task: String,
}

/// Spawn a wave dispatch via `POST /v0/waves/{wave}/dispatch` on a running
/// `lfd`. The child runs as its own attachable tmux session — monitor it
/// with `lfq sessions` / `lfq attach`.
pub fn dispatch_wave(options: &DispatchOptions) -> OpsResult<WaveRunDto> {
    let base_url = resolve_base_url();
    let client = blocking_client(DISPATCH_TIMEOUT)
        .map_err(|err| OpsError::Message(format!("failed to build http client: {err}")))?;
    let url = format!("{base_url}/v0/waves/{}/dispatch", options.wave);
    let body = serde_json::json!({
        "flow": options.flow,
        "task": options.task,
    });

    let response = authorize(client.post(&url))
        .json(&body)
        .send()
        .map_err(|err| OpsError::Message(format!("dispatch request to {url} failed: {err}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().unwrap_or_default();
        return Err(OpsError::Message(format!(
            "dispatch failed with HTTP {}: {}",
            status.as_u16(),
            text.trim()
        )));
    }

    response
        .json::<WaveRunDto>()
        .map_err(|err| OpsError::Message(format!("failed to parse dispatch response: {err}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::Json as JsonExtract;
    use axum::routing::post;
    use axum::Router;
    use std::sync::{Arc, Mutex};

    #[test]
    fn dispatch_wave_posts_flow_and_task_and_parses_response() {
        let captured: Arc<Mutex<Option<serde_json::Value>>> = Arc::new(Mutex::new(None));
        let captured_state = captured.clone();
        let app = Router::new().route(
            "/v0/waves/my-wave/dispatch",
            post(move |JsonExtract(body): JsonExtract<serde_json::Value>| {
                let captured_state = captured_state.clone();
                async move {
                    *captured_state.lock().expect("captured lock") = Some(body.clone());
                    axum::Json(serde_json::json!({
                        "id": "run_123",
                        "object": "wave_run",
                        "wave_id": "wave_123",
                        "flow": body.get("flow").and_then(|v| v.as_str()).unwrap_or_default(),
                        "task": body.get("task").and_then(|v| v.as_str()),
                        "repo": "/tmp/repo",
                        "direction": [],
                        "area": [],
                        "iteration": 0,
                        "step_index": 0,
                        "status": "pending",
                        "local_worktree": "/tmp/repo",
                        "remote_branch": "wave/my-wave",
                        "target_branch": "main",
                        "started_at": null,
                        "ended_at": null,
                        "error": null,
                        "flow_parents": [],
                        "stack_position": 0,
                        "pr_state_stale": false,
                        "created_at": null,
                    }))
                }
            }),
        );
        let addr = spawn_test_server(app);
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("LFD_URL", format!("http://{addr}"));

        let result = dispatch_wave(&DispatchOptions {
            wave: "my-wave".to_string(),
            flow: "implement".to_string(),
            task: "Add the dispatch endpoint.".to_string(),
        });

        std::env::remove_var("LFD_URL");
        let run = result.expect("dispatch succeeds");
        assert_eq!(run.flow, "implement");
        assert_eq!(run.task.as_deref(), Some("Add the dispatch endpoint."));

        let captured = captured.lock().expect("captured lock");
        let captured = captured.as_ref().expect("request captured");
        assert_eq!(
            captured.get("flow").and_then(|v| v.as_str()),
            Some("implement")
        );
        assert_eq!(
            captured.get("task").and_then(|v| v.as_str()),
            Some("Add the dispatch endpoint.")
        );
    }

    #[test]
    fn dispatch_wave_surfaces_error_response() {
        let app = Router::new().route(
            "/v0/waves/my-wave/dispatch",
            post(|| async {
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    "flow is required".to_string(),
                )
            }),
        );
        let addr = spawn_test_server(app);
        let _guard = env_lock().lock().expect("env lock");
        std::env::set_var("LFD_URL", format!("http://{addr}"));

        let result = dispatch_wave(&DispatchOptions {
            wave: "my-wave".to_string(),
            flow: String::new(),
            task: "task".to_string(),
        });

        std::env::remove_var("LFD_URL");
        let err = result.expect_err("dispatch should fail");
        assert!(err.to_string().contains("HTTP 400"));
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: Mutex<()> = Mutex::new(());
        &LOCK
    }

    /// Spawn `app` on a background thread with its own runtime, so a
    /// synchronous test can drive `dispatch_wave`'s blocking client against it.
    fn spawn_test_server(app: Router) -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind listener");
        listener.set_nonblocking(true).expect("set nonblocking");
        let addr = listener.local_addr().expect("listener addr");
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("server runtime");
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).expect("tokio listener");
                axum::serve(listener, app).await.expect("serve test app");
            });
        });
        addr
    }
}
