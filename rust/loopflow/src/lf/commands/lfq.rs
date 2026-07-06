//! `lfq run <lf command…>` — run an `lf` command on the wave server instead of
//! locally. `lfq` IS `lf`, remote: it forwards the argv verbatim to the wave's
//! `POST /exec` door (reusing the same endpoint + token machinery `lf chat`
//! speaks through), and the server runs `lf <argv>` **unsandboxed**, streaming
//! stdout/stderr/exit back.
//!
//! The point: a sandboxed harness (Codex `workspace-write`, Claude Code's bash
//! sandbox) can't write the main repo's `.git`, so `lf … --dispatch` and other
//! git-mutating ops fail on it. `lfq run` moves the *same* command to the
//! server, where nothing is sandboxed — `lf`'s semantics are unchanged, only
//! where it runs.
//!
//! Unlike `lf chat`, `lfq` never falls back to local when no server answers —
//! it errors. `lf` is the local door; `lfq` is the remote one.

use std::path::Path;

use anyhow::{anyhow, bail, Result};
use futures_util::StreamExt;

use crate::lf::commands::chat::{resolve_target, CliContext};
use crate::lf::WaveTargetArgs;
use crate::wave::server::read_resident_token;
use crate::wave::wire::{ExecFrame, ExecRequest, RESIDENT_TOKEN_ENV, RESIDENT_TOKEN_HEADER};

/// Run the forwarded `lf` command line on the wave server. `argv` is
/// everything after `lfq run`. Returns the remote `lf`'s exit code.
pub fn run(argv: &[String]) -> Result<i32> {
    if argv.is_empty() {
        bail!("usage: lfq run <lf command…>");
    }
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(run_async(argv))
}

async fn run_async(argv: &[String]) -> Result<i32> {
    // Which server: the wave named in the forwarded args, else the ambient one
    // — same resolution `lf chat`/`lf memory` use, so `--wave` behaves here
    // exactly as it does on a local `lf` invocation.
    let target = WaveTargetArgs {
        wave: wave_flag(argv),
        parent: false,
    };
    let context = CliContext::detect().await;
    let resolved = resolve_target(
        &target,
        context.store.as_ref(),
        context.repo.as_deref(),
        context.env_wave_id.as_deref(),
        context.env_channel.as_deref(),
    )
    .await?
    .ok_or_else(|| {
        anyhow!("no wave here — pass --wave <name> so lfq knows which server to run on")
    })?;

    let endpoint = resolved.require_endpoint()?;
    let token = resolve_token(resolved.repo_root.as_deref(), &resolved.name)?;
    let cwd = std::env::current_dir()?.to_string_lossy().into_owned();
    let body = ExecRequest {
        argv: argv.to_vec(),
        cwd,
    };
    stream_exec(&endpoint, &token, &body).await
}

/// The resident token, from the env the listener sets on a spawned resident,
/// else the token file beside the wave's endpoint pointer (readable because a
/// sandboxed mind can still *read* the main repo).
fn resolve_token(repo_root: Option<&Path>, wave: &str) -> Result<String> {
    if let Ok(token) = std::env::var(RESIDENT_TOKEN_ENV) {
        if !token.is_empty() {
            return Ok(token);
        }
    }
    let root = repo_root
        .ok_or_else(|| anyhow!("cannot locate the wave repo to read its resident token"))?;
    read_resident_token(root, wave).ok_or_else(|| {
        anyhow!("no resident token for wave '{wave}' — is `lf wave {wave}` running?")
    })
}

/// POST `/exec` and replay the streamed frames to our own stdout/stderr,
/// returning the child's exit code.
async fn stream_exec(endpoint: &str, token: &str, body: &ExecRequest) -> Result<i32> {
    let response = reqwest::Client::new()
        .post(format!("http://{endpoint}/exec"))
        .header(RESIDENT_TOKEN_HEADER, token)
        .json(body)
        .send()
        .await
        .map_err(|err| {
            anyhow!("wave server at {endpoint} is not answering ({err}) — is `lf wave` running?")
        })?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        bail!("wave server rejected /exec ({status}): {text}");
    }

    let mut stream = response.bytes_stream();
    let mut buf: Vec<u8> = Vec::new();
    let mut exit_code = None;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|err| anyhow!("exec stream error: {err}"))?;
        buf.extend_from_slice(&chunk);
        while let Some(nl) = buf.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = buf.drain(..=nl).collect();
            if let Some(code) = apply_frame(&line[..line.len() - 1]) {
                exit_code = Some(code);
            }
        }
    }
    // The Exit frame is the contract for completion; its absence means the
    // server died mid-stream — surface that rather than a silent success.
    exit_code.ok_or_else(|| anyhow!("exec stream ended before the child reported an exit code"))
}

/// Decode one frame line, echo stdout/stderr, and return an exit code if this
/// was the terminal frame. Malformed lines are ignored — a forward-compatible
/// server may add frame kinds this client doesn't know.
fn apply_frame(line: &[u8]) -> Option<i32> {
    if line.is_empty() {
        return None;
    }
    match serde_json::from_slice::<ExecFrame>(line) {
        Ok(ExecFrame::Stdout { data }) => {
            println!("{data}");
            None
        }
        Ok(ExecFrame::Stderr { data }) => {
            eprintln!("{data}");
            None
        }
        Ok(ExecFrame::Exit { code }) => Some(code),
        Err(_) => None,
    }
}

/// Pull `--wave <name>` / `--wave=<name>` out of the forwarded args to pick the
/// server. The flag still rides through to the remote `lf` untouched.
fn wave_flag(argv: &[String]) -> Option<String> {
    let mut args = argv.iter();
    while let Some(arg) = args.next() {
        if arg == "--wave" || arg == "-w" {
            return args.next().cloned();
        }
        if let Some(value) = arg.strip_prefix("--wave=") {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::wave_flag;

    #[test]
    fn wave_flag_reads_all_forms() {
        let cases = [
            (vec!["implement", "task", "--wave", "goals"], Some("goals")),
            (vec!["implement", "--wave=goals", "task"], Some("goals")),
            (vec!["implement", "-w", "goals"], Some("goals")),
            (vec!["implement", "task", "--dispatch"], None),
        ];
        for (argv, want) in cases {
            let argv: Vec<String> = argv.into_iter().map(str::to_string).collect();
            assert_eq!(wave_flag(&argv).as_deref(), want);
        }
    }
}
