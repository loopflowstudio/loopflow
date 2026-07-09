//! `POST /v0/exec` — the exec backdoor (lfd side).
//!
//! NOT the general path. Capabilities are normally `lf` run directly by
//! whoever needs them (daemonless). This door is the narrow escape hatch for
//! callers that genuinely cannot run `lf` in the needed context — chiefly the
//! future remote `lfq` client (M3), which terminates here. It takes an
//! arbitrary `lf` argv, validates it *parses* against the clap command tree
//! (invalid → 400, no exec), and if valid execs `lf` under the caller's
//! authority, returning exit code + streams. See scratch/lfd-exec.md for the
//! in-wave subagent → outwave listener variant and the `lfq` client.
//!
//! Authority: the route lives under the auth-protected `/v0` nest, so the
//! caller's capability token (loopback Bearer) is already verified by the time
//! a handler runs. The door adds no new trust — it execs exactly what an
//! authorized local caller could run as `lf`.
//!
//! Safety: argv is passed straight to the binary via `Command::args` — no
//! shell, so no shell-injection surface (see `crate::lfd::lf_exec`).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, ApiMessage, ApiResult};
use crate::lfd::lf_exec::{exec_lf, validate_lf_argv};

/// Request body for `/v0/exec`. `argv` is the command line after the `lf`
/// binary name (e.g. `["next", "--create-pr"]`). `cwd` is where to run
/// it — the caller owns resolving the wave worktree, since `lf` verbs infer
/// the wave from their working directory; omitted means lfd's own cwd.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecRequest {
    pub argv: Vec<String>,
    pub cwd: Option<String>,
}

/// Result of a door call. A non-zero `exit_code` is a *successful* door call
/// reporting a failed `lf` run; a refused exec (argv did not parse) never
/// reaches here — the caller gets a 400 instead.
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub async fn exec_handler(
    State(_state): State<HttpState>,
    Json(payload): Json<ExecRequest>,
) -> ApiResult<ExecResponse> {
    // Gate: the argv must parse as a real `lf` command. Garbage is refused
    // before anything is spawned.
    validate_lf_argv(&payload.argv)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, ApiMessage::Untrusted(err)))?;

    let result = exec_lf(&payload.argv, payload.cwd.as_deref(), &[])
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiMessage::Untrusted(err),
            )
        })?;

    Ok(Json(ExecResponse {
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::test_http_state;

    #[tokio::test]
    async fn invalid_argv_is_rejected_before_exec() {
        let state = test_http_state().await;
        let err = exec_handler(
            State(state),
            Json(ExecRequest {
                argv: vec!["next".to_string(), "--nonesuch".to_string()],
                cwd: None,
            }),
        )
        .await
        .expect_err("unknown flag must be refused");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn empty_argv_is_rejected() {
        let state = test_http_state().await;
        let err = exec_handler(
            State(state),
            Json(ExecRequest {
                argv: vec![],
                cwd: None,
            }),
        )
        .await
        .expect_err("bare lf is not a runnable command");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    /// Valid argv reaches the exec path and returns a structured result. `lf
    /// doctor` is read-only (dependency check) — a safe verb to actually
    /// run against the real binary in-tree via `CARGO_BIN_EXE_lf`.
    /// The door lives inside the auth-protected `/v0` nest: a request with no
    /// bearer token is refused before any exec. With the token it clears auth
    /// and reaches argv validation (garbage → 400). Proves the capability
    /// gate end-to-end through the real router stack.
    #[tokio::test]
    async fn exec_requires_bearer_token() {
        use tokio::net::TcpListener;

        let state = test_http_state().await;
        let app = crate::lfd::http::router(state);
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let _server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve app");
        });
        let url = format!("http://{addr}/v0/exec");
        let body = serde_json::json!({ "argv": ["doctor"] });
        let client = reqwest::Client::new();

        let unauthorized = client
            .post(&url)
            .json(&body)
            .send()
            .await
            .expect("request without token");
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

        // With the token, auth passes; a garbage argv proves we got past the
        // gate to the validator (400, not 401).
        let bad_argv = serde_json::json!({ "argv": ["next", "--nonesuch"] });
        let authorized = client
            .post(&url)
            .bearer_auth("test-token")
            .json(&bad_argv)
            .send()
            .await
            .expect("request with token");
        assert_eq!(authorized.status(), StatusCode::BAD_REQUEST);
    }
}
