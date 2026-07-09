//! `lfq` — the thin exec-door client. `lfq exec <lf argv…>` resolves a target
//! lfd endpoint, POSTs the argv to its `/v0/exec`, prints the captured
//! streams, and propagates the `lf` exit code. Generic passthrough: whatever
//! argv it is handed is forwarded whole — no per-subcommand mirror.
//!
//! The intended caller is a sandboxed subagent running inside a wave: the wave
//! injects `LF_WAVE_ENDPOINT` and a per-subagent token (`LF_SUBAGENT_TOKEN`)
//! into the subagent's env, so `lfq exec commit -m "…"` runs `lf`
//! unsandboxed in the outwave, escaping the subagent worktree's `.git`-write
//! restriction. Endpoint resolution is **env first, lfdb second**: the env
//! points a sandboxed process straight at its wave; the store is the fallback
//! for an external caller that knows the wave but not the port.

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};

use crate::lfd::http::routes::exec::{ExecRequest, ExecResponse};
use crate::lfd::types::WAVE_SERVER_ENDPOINT_ENV;
use crate::lfdb::{open_existing_store, SharedStore};
use crate::wave::wire::{SUBAGENT_TOKEN_ENV, SUBAGENT_TOKEN_HEADER};

/// The endpoint from `LF_WAVE_ENDPOINT`, trimmed; `None` when unset or empty.
fn env_endpoint() -> Option<String> {
    std::env::var(WAVE_SERVER_ENDPOINT_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// The per-subagent token from `LF_SUBAGENT_TOKEN`, trimmed.
fn subagent_token() -> Option<String> {
    std::env::var(SUBAGENT_TOKEN_ENV)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Resolve the target lfd endpoint: `LF_WAVE_ENDPOINT` first (a sandboxed
/// process reads its own env), then lfdb — the ambient wave's live server row.
pub async fn resolve_endpoint() -> Result<String> {
    if let Some(endpoint) = env_endpoint() {
        return Ok(endpoint);
    }
    resolve_endpoint_from_lfdb()
        .await
        .transpose()
        .unwrap_or_else(|| {
            Err(anyhow!(
                "no exec endpoint: set {WAVE_SERVER_ENDPOINT_ENV}, or run inside a wave with a live server"
            ))
        })
}

/// The lfdb fallback: resolve the ambient wave from the repo, then read its
/// live server endpoint off the registry. `Ok(None)` when there is no wave
/// context or no live server for it — the caller turns that into an error.
async fn resolve_endpoint_from_lfdb() -> Result<Option<String>> {
    let repo_root = crate::engine::repo::find_repo_root()
        .context("resolving the ambient wave for the exec endpoint")?;
    let main_repo =
        crate::engine::worktrees::main_repo_root(&repo_root).unwrap_or_else(|_| repo_root.clone());
    let Some(name) = crate::ops::util::resolve_wave_name(&main_repo, None) else {
        return Ok(None);
    };
    let Some(store) = open_existing_store().await else {
        return Ok(None);
    };
    let store: SharedStore = Arc::new(store);
    let Some(wave) = store.get_wave_by_name(&name).await? else {
        return Ok(None);
    };
    crate::wave::registry::wave_server_endpoint(&store, wave.id()).await
}

/// POST `argv` to `http://{endpoint}/v0/exec` with the subagent token header.
/// The wave door pins execution to the outwave, so `cwd` rides the request for
/// wire-shape parity with the machine lfd route (which honors it) but the wave
/// ignores it — send the caller's cwd honestly regardless.
async fn exec_request(
    endpoint: &str,
    token: &str,
    argv: Vec<String>,
    cwd: Option<String>,
) -> Result<ExecResponse> {
    let url = format!("http://{endpoint}/v0/exec");
    let response = reqwest::Client::new()
        .post(&url)
        .header(SUBAGENT_TOKEN_HEADER, token)
        .json(&ExecRequest { argv, cwd })
        .send()
        .await
        .with_context(|| format!("POST {url}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!("exec door refused ({status}): {body}"));
    }
    serde_json::from_str(&body).with_context(|| format!("decoding exec response: {body}"))
}

/// `lfq exec <argv…>`: resolve endpoint + token, run the argv through the
/// door, print the captured streams, and return the `lf` exit code for the
/// process to propagate.
pub async fn run(argv: Vec<String>) -> Result<i32> {
    if argv.is_empty() {
        return Err(anyhow!("usage: lfq exec <lf argv…>"));
    }
    let endpoint = resolve_endpoint().await?;
    let token = subagent_token().ok_or_else(|| {
        anyhow!("no {SUBAGENT_TOKEN_ENV} in env: lfq exec must run inside a wave subagent")
    })?;
    let cwd = std::env::current_dir()
        .ok()
        .map(|dir| dir.display().to_string());
    let result = exec_request(&endpoint, &token, argv, cwd).await?;
    print!("{}", result.stdout);
    eprint!("{}", result.stderr);
    Ok(result.exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serialize the tests that mutate `LF_WAVE_ENDPOINT` (process-global env).
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Env wins: `LF_WAVE_ENDPOINT` short-circuits before any lfdb lookup.
    // The guard is deliberately held across the `.await`: the env var must
    // stay set for the duration of the resolution, and no other test may
    // race it. Resolution short-circuits on the env, so nothing blocks.
    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn resolve_endpoint_prefers_env() {
        let _guard = env_lock();
        let previous = std::env::var_os(WAVE_SERVER_ENDPOINT_ENV);
        std::env::set_var(WAVE_SERVER_ENDPOINT_ENV, "127.0.0.1:4242");

        let endpoint = resolve_endpoint().await.expect("env endpoint resolves");
        assert_eq!(endpoint, "127.0.0.1:4242");

        match previous {
            Some(value) => std::env::set_var(WAVE_SERVER_ENDPOINT_ENV, value),
            None => std::env::remove_var(WAVE_SERVER_ENDPOINT_ENV),
        }
    }
}
