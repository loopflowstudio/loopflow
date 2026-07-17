//! `lf ssh <host> -- <cmd...>` — run a command on a remote home carrying the
//! caller's *local* credentials, resolved per invocation.
//!
//! The "bring your auth with you" model: credentials are resolved on this
//! machine, forwarded into the remote process environment as a stdin preamble
//! (never in argv, `ps`, or logs), and are NOT persisted on the remote — they
//! die with the process. The remote host stays a stateless compute surface.
//! Detached work is rejected when an account bundle is forwarded because it
//! would outlive that credential lease.
//!
//! Forwarded bundle: GitHub (`gh`), Claude/Codex agent OAuth, and — the
//! capability beyond the shell prototype — the PM/Linear token, which lives in
//! store rather than the environment. The remote `resolve_pm_token` reads
//! `LF_FORWARDED_PM_TOKEN` before its (empty) store, so remote `lf pm` works.
//!
//! Secrets policy: `lf ssh` forwards specific resolved secrets, never the
//! Doppler token that could fetch them all. The Doppler login/CLI token is a
//! master key to the whole secret estate and never leaves this machine. When a
//! remote command needs a Doppler-backed secret, name it with `--secret NAME`:
//! the value is resolved *locally* via Doppler and only that value is forwarded.
//! Agent forwarding (`ssh -A`) is off by default — git pushes ride the
//! forwarded `GH_TOKEN` over HTTPS, so the caller's SSH identity stays home.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context};

use crate::pm::PmProviderKind;
use crate::provider_account::{
    encode_forwarded_account_bundle, local_forwarded_account_bundle, ForwardedAccountBundle,
    FORWARDED_ACCOUNT_BUNDLE_ENV, FORWARDED_ACCOUNT_STORE_ENV, PROVIDER_ACCOUNT_ENV,
};
use crate::provider_auth::{extract_claude_token, extract_codex_access_token};

/// Default repository path (relative to `$HOME`) the remote command runs in.
pub const DEFAULT_REPO: &str = "src/loopflow";

/// The local credential bundle forwarded to the remote. Absent credentials are
/// simply not exported — the remote falls back to whatever it can resolve.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub gh_token: Option<String>,
    pub claude_token: Option<String>,
    pub codex_token: Option<String>,
    pub account_bundle: Option<ForwardedAccountBundle>,
    pub pm_token: Option<String>,
    /// PM provider the token belongs to (e.g. `linear`).
    pub pm_provider: Option<String>,
    /// Doppler-backed secrets resolved locally, forwarded as `export NAME=value`.
    pub secrets: Vec<(String, String)>,
}

/// Run `cmd` on `host` in `$HOME/<repo>` with the local credential bundle
/// forwarded. Propagates the remote exit code.
///
/// `secret_names` are resolved locally via Doppler and forwarded as env vars —
/// the sanctioned way a remote command receives a Doppler-backed secret without
/// the remote ever holding a Doppler credential.
///
/// `forward_agent` opts into `ssh -A`. It is off by default: git pushes ride
/// the forwarded `GH_TOKEN` over HTTPS, so agent forwarding is dead weight that
/// would hand the caller's whole SSH identity to the remote.
pub fn run(
    dest: &str,
    repo: Option<&str>,
    secret_names: &[String],
    forward_agent: bool,
    cmd: &[String],
) -> anyhow::Result<()> {
    if cmd.is_empty() {
        return Err(anyhow!(
            "lf ssh needs a command after `--`, e.g. `lf ssh {dest} -- lf pr open`"
        ));
    }
    run_with_env(dest, None, repo, secret_names, forward_agent, cmd, &[])
}

/// Run a Wave-home-routed `lf` command on `dest` (an `owner@host` destination)
/// with an optional `port`, exporting `LF_HOME_ROUTED=1` so the remote `lf` runs
/// the command locally instead of routing it back to its own home — the single
/// break in the forward loop. Reuses the same credential-forwarding preamble as
/// `lf ssh`; no new transport or secret path.
pub fn run_routed(
    dest: &str,
    port: Option<u16>,
    repo: Option<&str>,
    cmd: &[String],
) -> anyhow::Result<()> {
    run_with_env(
        dest,
        port,
        repo,
        &[],
        false,
        cmd,
        &[(crate::engine::wave_home::HOME_ROUTED_ENV, "1")],
    )
}

/// Run a routed `lf` command on `dest`/`port` and capture its stdout, used by
/// the Home probe to read a remote `lf status --json` over the same SSH and
/// credential machinery. On a transport failure returns [`SshCaptureError`] so
/// the caller can classify unreachable distinctly from a command that ran but
/// answered unexpectedly.
pub fn capture_routed(
    dest: &str,
    port: Option<u16>,
    cmd: &[String],
) -> Result<String, SshCaptureError> {
    let repo = DEFAULT_REPO;
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|error| SshCaptureError::Local(error.to_string()))?;
    let credentials = runtime
        .block_on(resolve_credentials(&[]))
        .map_err(|error| SshCaptureError::Local(error.to_string()))?;
    reject_detached_account_forwarding(credentials.account_bundle.is_some(), cmd)
        .map_err(|error| SshCaptureError::Local(error.to_string()))?;
    let preamble = build_preamble(
        &credentials,
        dest,
        repo,
        cmd,
        &[(crate::engine::wave_home::HOME_ROUTED_ENV, "1")],
    );
    run_ssh_capture(dest, port, &preamble)
}

/// Why a captured SSH command did not yield usable stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshCaptureError {
    /// The SSH transport failed (unreachable host, refused auth, timeout).
    Unreachable(String),
    /// The remote command ran but exited nonzero.
    Command { code: i32, stderr: String },
    /// A local failure before ssh (runtime, credential resolution).
    Local(String),
}

fn run_with_env(
    dest: &str,
    port: Option<u16>,
    repo: Option<&str>,
    secret_names: &[String],
    forward_agent: bool,
    cmd: &[String],
    extra_env: &[(&str, &str)],
) -> anyhow::Result<()> {
    let repo = repo.unwrap_or(DEFAULT_REPO);
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let credentials = runtime.block_on(resolve_credentials(secret_names))?;
    reject_detached_account_forwarding(credentials.account_bundle.is_some(), cmd)?;
    let preamble = build_preamble(&credentials, dest, repo, cmd, extra_env);
    run_ssh(dest, port, forward_agent, &preamble)
}

fn reject_detached_account_forwarding(
    has_account_bundle: bool,
    cmd: &[String],
) -> anyhow::Result<()> {
    if has_account_bundle && cmd.first().is_some_and(|program| program == "tmux") {
        return Err(anyhow!(
            "cannot forward ephemeral provider accounts into a remote tmux command; \
             run the remote command in the foreground or authenticate on the remote host"
        ));
    }
    Ok(())
}

/// Resolve the credential bundle from local sources. Auth tokens that aren't
/// present resolve to `None`; a `--secret` that can't be resolved is a hard
/// error (the caller explicitly asked for it). Nothing here prints a value.
async fn resolve_credentials(secret_names: &[String]) -> anyhow::Result<Credentials> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut secrets = Vec::with_capacity(secret_names.len());
    for name in secret_names {
        secrets.push((name.clone(), resolve_doppler_secret(name)?));
    }
    let selector = std::env::var(PROVIDER_ACCOUNT_ENV).ok();
    if selector
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(anyhow!(
            "--account requires a non-empty account id or login email"
        ));
    }
    let account_bundle = match crate::store::open_existing_store().await {
        Some(store) => {
            local_forwarded_account_bundle(&std::sync::Arc::new(store), selector.as_deref()).await?
        }
        None if selector.is_some() => {
            return Err(anyhow!("account store unavailable for --account"));
        }
        None => None,
    };
    let (claude_token, codex_token) = if account_bundle.is_some() {
        (None, None)
    } else {
        (
            extract_claude_token(&home).map(|token| token.access_token),
            extract_codex_access_token(&home),
        )
    };
    Ok(Credentials {
        gh_token: resolve_gh_token(),
        claude_token,
        codex_token,
        account_bundle,
        pm_token: resolve_pm_token().await,
        pm_provider: Some(PmProviderKind::Linear.as_str().to_string()),
        secrets,
    })
}

/// GitHub token via the `gh` CLI (honors `GH_TOKEN`, refreshes when needed).
fn resolve_gh_token() -> Option<String> {
    let output = Command::new("gh").args(["auth", "token"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!token.is_empty()).then_some(token)
}

/// Resolve one secret from Doppler on the local machine. The Doppler token never
/// leaves this process; only the resolved value is forwarded.
fn resolve_doppler_secret(name: &str) -> anyhow::Result<String> {
    if !is_valid_env_name(name) {
        return Err(anyhow!(
            "invalid --secret name '{name}': expected an environment variable identifier"
        ));
    }
    let output = Command::new("doppler")
        .args(["secrets", "get", name, "--plain"])
        .output()
        .with_context(|| format!("failed to run doppler for secret '{name}'"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "doppler could not resolve secret '{name}' (is it set in the active config?)"
        ));
    }
    // Strip only the trailing newline doppler appends; keep the value otherwise.
    let value = String::from_utf8(output.stdout)
        .with_context(|| format!("doppler returned non-UTF8 value for '{name}'"))?
        .trim_end_matches(['\n', '\r'])
        .to_string();
    if value.is_empty() {
        return Err(anyhow!(
            "doppler returned an empty value for secret '{name}'"
        ));
    }
    Ok(value)
}

/// PM/Linear access token from the local store credential store. Absent when no
/// store exists or no Linear credential is stored.
async fn resolve_pm_token() -> Option<String> {
    let cfg = crate::store::storage_config_from_env().ok()?;
    let store = crate::store::open_store(&cfg).await.ok()?;
    let token = store
        .get_provider_token(PmProviderKind::Linear.as_str())
        .await
        .ok()??;
    Some(token.access_token)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// A valid POSIX environment variable name: `[A-Za-z_][A-Za-z0-9_]*`.
fn is_valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Assemble the bash preamble piped to the remote over stdin. Every secret value
/// is single-quote escaped so it can never break out of the assignment; the
/// values travel only through this channel, never through argv.
fn build_preamble(
    credentials: &Credentials,
    host: &str,
    repo: &str,
    cmd: &[String],
    extra_env: &[(&str, &str)],
) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Give the remote a sane PATH so `lf`, `gh` resolve under a
    // non-interactive `bash -s`.
    lines.push(
        "export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\""
            .to_string(),
    );

    // Router-supplied markers (e.g. LF_HOME_ROUTED) exported before creds so the
    // remote sees them regardless of which credentials resolve.
    for (name, value) in extra_env {
        lines.push(format!("export {name}={}", sh_quote(value)));
    }

    if let Some(token) = nonempty(&credentials.gh_token) {
        lines.push(format!("export GH_TOKEN={}", sh_quote(token)));
        // Route HTTPS git pushes through the forwarded token via env-only config
        // (nothing written to the remote ~/.gitconfig). Two entries:
        //  - reset `credential.helper` to empty first, so any ambient global
        //    helper on the remote (store/cache/osxkeychain) can't fire and
        //    persist the token to disk — preserving "nothing persisted".
        //  - then install our in-memory helper. The `$GH_TOKEN` inside it is
        //    evaluated on the remote when git invokes it; the single quotes keep
        //    it literal here, exactly as intended.
        lines.push("export GIT_CONFIG_COUNT=2".to_string());
        lines.push(format!(
            "export GIT_CONFIG_KEY_0={}",
            sh_quote("credential.helper")
        ));
        lines.push("export GIT_CONFIG_VALUE_0=''".to_string());
        lines.push(format!(
            "export GIT_CONFIG_KEY_1={}",
            sh_quote("credential.https://github.com.helper")
        ));
        lines.push(format!(
            "export GIT_CONFIG_VALUE_1={}",
            sh_quote("!f(){ echo username=x-access-token; echo \"password=$GH_TOKEN\"; };f")
        ));
    }
    if let Some(token) = nonempty(&credentials.claude_token) {
        lines.push(format!(
            "export CLAUDE_CODE_OAUTH_TOKEN={}",
            sh_quote(token)
        ));
    }
    if let Some(token) = nonempty(&credentials.codex_token) {
        lines.push(format!("export CODEX_ACCESS_TOKEN={}", sh_quote(token)));
    }
    if let Some(account_bundle) = &credentials.account_bundle {
        match encode_forwarded_account_bundle(account_bundle) {
            Ok(bundle) => lines.push(format!(
                "export {FORWARDED_ACCOUNT_BUNDLE_ENV}={}",
                sh_quote(&bundle)
            )),
            Err(error) => lines.push(format!(
                "echo {} >&2; exit 1",
                sh_quote(&format!("could not encode provider accounts: {error}"))
            )),
        }
        lines.push(
            "LF_ACCOUNT_LEASE_DIR=$(mktemp -d \"${TMPDIR:-/tmp}/lf-account.XXXXXX\") || exit 1"
                .to_string(),
        );
        lines.push("export LF_ACCOUNT_LEASE_DIR".to_string());
        lines.push(format!(
            "export {FORWARDED_ACCOUNT_STORE_ENV}=\"$LF_ACCOUNT_LEASE_DIR/router.db\""
        ));
        lines.push(
            "trap 'status=$?; trap - EXIT; rm -rf -- \"$LF_ACCOUNT_LEASE_DIR\"; exit \"$status\"' EXIT"
                .to_string(),
        );
        lines.push("trap 'exit 129' HUP".to_string());
        lines.push("trap 'exit 130' INT".to_string());
        lines.push("trap 'exit 143' TERM".to_string());
    }
    if let Some(token) = nonempty(&credentials.pm_token) {
        lines.push(format!("export LF_FORWARDED_PM_TOKEN={}", sh_quote(token)));
        if let Some(provider) = nonempty(&credentials.pm_provider) {
            lines.push(format!(
                "export LF_FORWARDED_PM_PROVIDER={}",
                sh_quote(provider)
            ));
        }
    }
    // Doppler-backed secrets resolved locally: only the value crosses the wire.
    for (name, value) in &credentials.secrets {
        lines.push(format!("export {name}={}", sh_quote(value)));
    }

    // cd into the repo; `$HOME` expands on the remote, the path stays quoted.
    lines.push(format!(
        "cd \"$HOME\"/{} || {{ echo {} >&2; exit 1; }}",
        sh_quote(repo),
        sh_quote(&format!("no repo ~/{repo} on {host}"))
    ));

    let remote_cmd = cmd
        .iter()
        .map(|arg| sh_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    lines.push(if credentials.account_bundle.is_some() {
        remote_cmd
    } else {
        format!("exec {remote_cmd}")
    });

    let mut preamble = lines.join("\n");
    preamble.push('\n');
    preamble
}

fn nonempty(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|value| !value.trim().is_empty())
}

/// POSIX single-quote escaping: wrap in `'…'`, and render any embedded single
/// quote as `'\''`. Safe for arbitrary bytes including secrets.
fn sh_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('\'');
    for ch in value.chars() {
        if ch == '\'' {
            quoted.push_str("'\\''");
        } else {
            quoted.push(ch);
        }
    }
    quoted.push('\'');
    quoted
}

/// Bound the connect handshake so an unreachable host fails fast instead of
/// riding the OS TCP timeout (minutes).
const CONNECT_TIMEOUT_SECS: u32 = 10;
/// Keepalive probe cadence and tolerance — bounds a stalled *established*
/// connection (dead network mid-session) to ~30s.
const SERVER_ALIVE_INTERVAL_SECS: u32 = 10;
const SERVER_ALIVE_COUNT_MAX: u32 = 3;

/// What the remote process's exit status means for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SshOutcome {
    /// Remote command succeeded.
    Success,
    /// SSH transport/connection failure (its reserved code `255`, or death by
    /// signal): unreachable host, unknown key, auth refusal, or a bounded
    /// timeout firing. Actionable and host-named; never a real remote code.
    ConnectionFailure,
    /// The remote command itself exited nonzero — propagate its code verbatim.
    CommandFailure(i32),
}

/// Build the `ssh` argument vector with noninteractive bounds. Pure so the
/// bounds are unit-testable without a live host.
///
/// `BatchMode=yes` is the primary hang killer: it refuses every interactive
/// prompt (password, passphrase, unknown host key) rather than blocking on the
/// tty forever. The timeouts bound the connect handshake and a stalled session.
fn ssh_args(dest: &str, port: Option<u16>, forward_agent: bool) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if forward_agent {
        args.push("-A".to_string());
    }
    if let Some(port) = port {
        args.push("-p".to_string());
        args.push(port.to_string());
    }
    args.push("-o".to_string());
    args.push("BatchMode=yes".to_string());
    args.push("-o".to_string());
    args.push(format!("ConnectTimeout={CONNECT_TIMEOUT_SECS}"));
    args.push("-o".to_string());
    args.push(format!("ServerAliveInterval={SERVER_ALIVE_INTERVAL_SECS}"));
    args.push("-o".to_string());
    args.push(format!("ServerAliveCountMax={SERVER_ALIVE_COUNT_MAX}"));
    args.push(dest.to_string());
    args.push("bash -s".to_string());
    args
}

/// Classify an ssh exit code. `255` is ssh's reserved transport-error code;
/// `None` means death by signal — both are connection-phase failures, distinct
/// from a real remote command code we must propagate.
fn classify_exit(code: Option<i32>) -> SshOutcome {
    match code {
        Some(0) => SshOutcome::Success,
        Some(255) | None => SshOutcome::ConnectionFailure,
        Some(other) => SshOutcome::CommandFailure(other),
    }
}

/// A sanitized, host-named error for a transport-phase failure. Carries no
/// credential value; ssh's own reason is already on the inherited stderr.
fn connection_error(host: &str) -> anyhow::Error {
    anyhow!(
        "lf ssh could not reach '{host}': ssh failed during connection/transport \
         (bounded by BatchMode + ConnectTimeout={CONNECT_TIMEOUT_SECS}s). See the ssh \
         error above; check the host is reachable, its key is known, and key auth works."
    )
}

/// Pipe the preamble into `ssh [-A] <host> bash -s`, streaming stdout/stderr and
/// propagating the remote exit code. Agent forwarding (`-A`) is opt-in. Bounded
/// so an unreachable or misconfigured host fails fast instead of hanging.
fn run_ssh(
    dest: &str,
    port: Option<u16>,
    forward_agent: bool,
    preamble: &str,
) -> anyhow::Result<()> {
    let mut child = Command::new("ssh")
        .args(ssh_args(dest, port, forward_agent))
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn ssh")?;

    child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("ssh stdin unavailable"))?
        .write_all(preamble.as_bytes())
        .context("failed to write preamble to ssh")?;

    let status = child.wait().context("ssh did not complete")?;
    match classify_exit(status.code()) {
        SshOutcome::Success => Ok(()),
        SshOutcome::ConnectionFailure => Err(connection_error(dest)),
        SshOutcome::CommandFailure(code) => std::process::exit(code),
    }
}

/// Like [`run_ssh`] but captures stdout (the preamble on stdin, stdout to a
/// buffer, stderr still inherited so ssh's own diagnostics stay visible).
/// Classifies transport failure vs. a nonzero remote command so the Home probe
/// can tell "unreachable" from "answered oddly".
fn run_ssh_capture(
    dest: &str,
    port: Option<u16>,
    preamble: &str,
) -> Result<String, SshCaptureError> {
    let mut child = Command::new("ssh")
        .args(ssh_args(dest, port, false))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| SshCaptureError::Local(format!("failed to spawn ssh: {error}")))?;

    child
        .stdin
        .take()
        .ok_or_else(|| SshCaptureError::Local("ssh stdin unavailable".to_string()))?
        .write_all(preamble.as_bytes())
        .map_err(|error| SshCaptureError::Local(format!("failed to write preamble: {error}")))?;

    let output = child
        .wait_with_output()
        .map_err(|error| SshCaptureError::Local(format!("ssh did not complete: {error}")))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match classify_exit(output.status.code()) {
        SshOutcome::Success => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        SshOutcome::ConnectionFailure => Err(SshCaptureError::Unreachable(
            connection_error(dest).to_string(),
        )),
        SshOutcome::CommandFailure(code) => Err(SshCaptureError::Command { code, stderr }),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;

    use tempfile::tempdir;

    use super::*;
    use crate::provider_account::ForwardedAccountSelection;

    struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvRestore {
        fn capture(names: &[&'static str]) -> Self {
            Self(
                names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    struct CurrentDirRestore(std::path::PathBuf);

    impl CurrentDirRestore {
        fn enter(path: &std::path::Path) -> Self {
            let current = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self(current)
        }
    }

    impl Drop for CurrentDirRestore {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.0).unwrap();
        }
    }

    fn full_bundle() -> Credentials {
        let claude = crate::provider_account::parse_account_id("primary").unwrap();
        let codex = crate::provider_account::parse_account_id("reserve").unwrap();
        Credentials {
            gh_token: Some("gh-secret".to_string()),
            claude_token: None,
            codex_token: None,
            account_bundle: Some(ForwardedAccountBundle::new(
                ForwardedAccountSelection::Routed {
                    repo_id: crate::repository::RepoId::parse("loopflowstudio/loopflow").unwrap(),
                    routes: vec![
                        crate::provider_account::ForwardedProviderRoute {
                            provider: crate::provider_auth::Provider::Claude,
                            accounts: vec![claude.clone()],
                        },
                        crate::provider_account::ForwardedProviderRoute {
                            provider: crate::provider_auth::Provider::Codex,
                            accounts: vec![codex.clone()],
                        },
                    ],
                },
                vec![
                    crate::provider_account::ForwardedProviderAccount {
                        provider: crate::provider_auth::Provider::Claude,
                        account_id: claude.clone(),
                        login_email: Some(
                            crate::profile::EmailAddress::parse("personal@example.com").unwrap(),
                        ),
                        credential_state: crate::store::CredentialState::Connected,
                        routing_state: crate::store::RoutingState::Automatic,
                        plan: Some("max".to_string()),
                        paid_through: None,
                        utilization_percent: None,
                        cooldown_until: None,
                        cooldown_reason: None,
                    },
                    crate::provider_account::ForwardedProviderAccount {
                        provider: crate::provider_auth::Provider::Codex,
                        account_id: codex.clone(),
                        login_email: Some(
                            crate::profile::EmailAddress::parse("engineering@example.com").unwrap(),
                        ),
                        credential_state: crate::store::CredentialState::Connected,
                        routing_state: crate::store::RoutingState::Automatic,
                        plan: Some("max".to_string()),
                        paid_through: None,
                        utilization_percent: None,
                        cooldown_until: None,
                        cooldown_reason: None,
                    },
                ],
                vec![
                    crate::provider_account::ForwardedProviderCredential::new(
                        crate::provider_auth::Provider::Claude,
                        claude,
                        "claude-primary".to_string(),
                    ),
                    crate::provider_account::ForwardedProviderCredential::new(
                        crate::provider_auth::Provider::Codex,
                        codex,
                        "codex-reserve".to_string(),
                    ),
                ],
            )),
            pm_token: Some("linear-secret".to_string()),
            pm_provider: Some("linear".to_string()),
            secrets: vec![("STRIPE_KEY".to_string(), "sk-live-123".to_string())],
        }
    }

    #[test]
    fn remote_tmux_rejects_ephemeral_account_forwarding() {
        let cmd = vec!["tmux".to_string(), "list-sessions".to_string()];

        assert!(reject_detached_account_forwarding(false, &cmd).is_ok());
        assert!(reject_detached_account_forwarding(true, &cmd)
            .unwrap_err()
            .to_string()
            .contains("remote tmux"));
    }

    #[test]
    fn preamble_exports_every_credential_and_execs_command() {
        let cmd = vec!["lf".to_string(), "op".to_string(), "pr".to_string()];
        let preamble = build_preamble(&full_bundle(), "mini-heart", "src/loopflow", &cmd, &[]);

        assert!(preamble.contains("export GH_TOKEN='gh-secret'"));
        assert!(!preamble.contains("export CLAUDE_CODE_OAUTH_TOKEN="));
        assert!(!preamble.contains("export CODEX_ACCESS_TOKEN="));
        let encoded =
            encode_forwarded_account_bundle(full_bundle().account_bundle.as_ref().unwrap())
                .unwrap();
        assert!(preamble.contains(&format!(
            "export {FORWARDED_ACCOUNT_BUNDLE_ENV}='{}'",
            encoded
        )));
        assert!(!preamble.contains("claude-primary"));
        assert!(!preamble.contains("codex-reserve"));
        assert!(!preamble.contains("refresh_token"));
        assert!(!preamble.contains("/accounts/"));
        assert!(preamble.contains("LF_ACCOUNT_LEASE_DIR=$(mktemp -d"));
        assert!(preamble.contains(&format!("export {FORWARDED_ACCOUNT_STORE_ENV}=")));
        assert!(preamble.contains("trap - EXIT; rm -rf --"));
        assert!(preamble.contains("export LF_FORWARDED_PM_TOKEN='linear-secret'"));
        assert!(preamble.contains("export LF_FORWARDED_PM_PROVIDER='linear'"));
        // Locally-resolved Doppler secret, forwarded by value; no Doppler token.
        assert!(preamble.contains("export STRIPE_KEY='sk-live-123'"));
        assert!(!preamble.contains("DOPPLER_TOKEN"));
        // git HTTPS credential helper wired to the forwarded token, with the
        // ambient helper list reset first so nothing persists to remote disk.
        assert!(preamble.contains("export GIT_CONFIG_COUNT=2"));
        assert!(preamble.contains("export GIT_CONFIG_KEY_0='credential.helper'"));
        assert!(preamble.contains("export GIT_CONFIG_VALUE_0=''"));
        assert!(preamble.contains("export GIT_CONFIG_KEY_1='credential.https://github.com.helper'"));
        assert!(preamble.contains("password=$GH_TOKEN"));
        // cd into the repo and run under the cleanup trap.
        assert!(preamble.contains("cd \"$HOME\"/'src/loopflow'"));
        assert!(preamble.trim_end().ends_with("'lf' 'op' 'pr'"));
    }

    #[allow(clippy::await_holding_lock)]
    #[test]
    fn foreground_account_lease_removes_its_store_on_command_exit() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let ssh = bin.join("ssh");
        fs::write(&ssh, "#!/bin/sh\nexec bash -s\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&ssh).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&ssh, permissions).unwrap();
        }
        let _restore = EnvRestore::capture(&["HOME", "PATH"]);
        std::env::set_var("HOME", temp.path());
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        let record = temp.path().join("lease-path");
        let cmd = vec![
            "sh".to_string(),
            "-c".to_string(),
            "printf '%s' \"$LF_ACCOUNT_LEASE_DIR\" > \"$LEASE_RECORD\"; : > \"$LF_FORWARDED_ACCOUNT_STORE\""
                .to_string(),
        ];
        let preamble = build_preamble(
            &full_bundle(),
            "host",
            ".",
            &cmd,
            &[("LEASE_RECORD", record.to_str().unwrap())],
        );
        run_ssh("host", None, false, &preamble).unwrap();
        let lease = PathBuf::from(fs::read_to_string(record).unwrap());
        assert!(!lease.exists());
        assert!(!lease.join("router.db").exists());
    }

    #[allow(clippy::await_holding_lock)]
    #[test]
    fn explicit_account_reaches_foreground_ssh_outside_a_repository() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let transport_input = temp.path().join("ssh-input");
        let ssh = bin.join("ssh");
        fs::write(
            &ssh,
            format!("#!/bin/sh\ncat > '{}'\n", transport_input.display()),
        )
        .unwrap();
        let codex = bin.join("codex");
        fs::write(
            &codex,
            r#"#!/bin/sh
IFS= read -r line
printf '{"id":1,"result":{}}\n'
IFS= read -r line
IFS= read -r line
printf '{"id":2,"result":{"account":null}}\n'
"#,
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            for executable in [&ssh, &codex] {
                let mut permissions = fs::metadata(executable).unwrap().permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(executable, permissions).unwrap();
            }
        }
        let _restore = EnvRestore::capture(&[
            "HOME",
            "LF_HOME",
            "LF_DB_PATH",
            "PATH",
            crate::provider_account::ACCOUNT_REPO_ID_ENV,
            PROVIDER_ACCOUNT_ENV,
            FORWARDED_ACCOUNT_BUNDLE_ENV,
            FORWARDED_ACCOUNT_STORE_ENV,
        ]);
        std::env::set_var("HOME", temp.path());
        std::env::set_var("LF_HOME", temp.path());
        let database = temp.path().join("loopflow.db");
        std::env::set_var("LF_DB_PATH", &database);
        std::env::remove_var(crate::provider_account::ACCOUNT_REPO_ID_ENV);
        std::env::remove_var(FORWARDED_ACCOUNT_BUNDLE_ENV);
        std::env::remove_var(FORWARDED_ACCOUNT_STORE_ENV);
        std::env::set_var(PROVIDER_ACCOUNT_ENV, "engineering");
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = std::sync::Arc::new(
            runtime
                .block_on(crate::store::open_store(
                    &crate::store::StorageConfig::sqlite(database),
                ))
                .unwrap(),
        );
        let account_home = temp.path().join("accounts/engineering");
        fs::create_dir_all(&account_home).unwrap();
        fs::write(
            account_home.join("auth.json"),
            r#"{"tokens":{"access_token":"e30.eyJleHAiOjQxMDI0NDQ4MDB9.sig","id_token":"e30.eyJlbWFpbCI6ImVuZ2luZWVyaW5nQGV4YW1wbGUuY29tIn0.sig"}}"#,
        )
        .unwrap();
        let account = crate::provider_account::new_account(
            crate::provider_auth::Provider::Codex,
            crate::provider_account::parse_account_id("engineering").unwrap(),
            account_home,
            None,
        );
        runtime
            .block_on(store.upsert_provider_account(&account))
            .unwrap();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let _cwd = CurrentDirRestore::enter(&outside);

        run("host", Some("."), &[], false, &["lf".to_string()]).unwrap();

        let input = fs::read_to_string(transport_input).unwrap();
        assert!(input.contains(FORWARDED_ACCOUNT_BUNDLE_ENV));
        assert!(input.trim_end().ends_with("'lf'"));
    }

    #[allow(clippy::await_holding_lock)]
    #[test]
    fn explicit_account_errors_before_ssh_transport() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        let marker = temp.path().join("ssh-spawned");
        let ssh = bin.join("ssh");
        fs::write(
            &ssh,
            format!("#!/bin/sh\n: > '{}'\nexit 99\n", marker.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&ssh).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&ssh, permissions).unwrap();
        }
        let _restore = EnvRestore::capture(&[
            "HOME",
            "LF_HOME",
            "LF_DB_PATH",
            "PATH",
            crate::provider_account::ACCOUNT_REPO_ID_ENV,
            PROVIDER_ACCOUNT_ENV,
            FORWARDED_ACCOUNT_BUNDLE_ENV,
            FORWARDED_ACCOUNT_STORE_ENV,
        ]);
        std::env::set_var("HOME", temp.path());
        std::env::set_var("LF_HOME", temp.path());
        let database = temp.path().join("loopflow.db");
        std::env::set_var("LF_DB_PATH", &database);
        std::env::remove_var(crate::provider_account::ACCOUNT_REPO_ID_ENV);
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        std::env::remove_var(FORWARDED_ACCOUNT_BUNDLE_ENV);
        std::env::remove_var(FORWARDED_ACCOUNT_STORE_ENV);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = std::sync::Arc::new(
            runtime
                .block_on(crate::store::open_store(
                    &crate::store::StorageConfig::sqlite(database),
                ))
                .unwrap(),
        );
        let command = ["lf".to_string()];
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let _cwd = CurrentDirRestore::enter(&outside);

        std::env::set_var(PROVIDER_ACCOUNT_ENV, "   ");
        let error = run("host", Some("."), &[], false, &command).unwrap_err();
        assert_eq!(
            error.to_string(),
            "--account requires a non-empty account id or login email"
        );
        assert!(!marker.exists());

        std::env::set_var(PROVIDER_ACCOUNT_ENV, "missing");
        let error = run("host", Some("."), &[], false, &command).unwrap_err();
        assert!(error
            .to_string()
            .contains("no managed Claude or Codex account matches 'missing'"));
        assert!(!marker.exists());

        for account_id in ["engineering-one", "engineering-two"] {
            let account = crate::provider_account::new_account(
                crate::provider_auth::Provider::Codex,
                crate::provider_account::parse_account_id(account_id).unwrap(),
                temp.path().join("accounts").join(account_id),
                None,
            );
            runtime
                .block_on(store.upsert_provider_account(&account))
                .unwrap();
        }
        std::env::set_var(PROVIDER_ACCOUNT_ENV, "engineering");
        let error = run("host", Some("."), &[], false, &command).unwrap_err();
        assert!(error.to_string().contains(
            "'engineering' matches several codex accounts: engineering-one, engineering-two"
        ));
        assert!(!marker.exists());

        let offline = crate::provider_account::new_account(
            crate::provider_auth::Provider::Codex,
            crate::provider_account::parse_account_id("offline").unwrap(),
            temp.path().join("accounts/offline"),
            None,
        );
        runtime
            .block_on(store.upsert_provider_account(&offline))
            .unwrap();
        std::env::set_var(PROVIDER_ACCOUNT_ENV, "offline");
        let error = run("host", Some("."), &[], false, &command).unwrap_err();
        assert!(error.to_string().contains(
            "no authenticated codex account remains; reconnect 'offline' with `lf auth connect codex offline`"
        ));
        assert!(!marker.exists());
    }

    #[test]
    fn preamble_omits_absent_credentials() {
        let creds = Credentials {
            claude_token: Some("only-claude".to_string()),
            ..Credentials::default()
        };
        let cmd = vec!["lf".to_string(), "runs".to_string()];
        let preamble = build_preamble(&creds, "host", "src/loopflow", &cmd, &[]);

        assert!(preamble.contains("export CLAUDE_CODE_OAUTH_TOKEN='only-claude'"));
        assert!(!preamble.contains("GH_TOKEN"));
        assert!(!preamble.contains("LF_FORWARDED_PM_TOKEN"));
        assert!(!preamble.contains("GIT_CONFIG_COUNT"));
        assert!(!preamble.contains("credential.helper"));
        assert!(!preamble.contains("LF_HOME_ROUTED"));
    }

    #[test]
    fn routed_preamble_marks_the_home_hop_to_break_the_forward_loop() {
        let cmd = vec!["lf".to_string(), "pr".to_string(), "open".to_string()];
        let preamble = build_preamble(
            &Credentials::default(),
            "mini-heart",
            "src/loopflow",
            &cmd,
            &[("LF_HOME_ROUTED", "1")],
        );
        assert!(preamble.contains("export LF_HOME_ROUTED='1'"));
        assert!(preamble.trim_end().ends_with("exec 'lf' 'pr' 'open'"));
    }

    #[test]
    fn preamble_never_leaks_a_secret_to_argv_form() {
        // A secret containing shell metacharacters stays inside single quotes,
        // so it can neither break the assignment nor reach a command position.
        let creds = Credentials {
            gh_token: Some("a'b; rm -rf ~ #".to_string()),
            ..Credentials::default()
        };
        let cmd = vec!["lf".to_string()];
        let preamble = build_preamble(&creds, "host", "src/loopflow", &cmd, &[]);

        assert!(preamble.contains(r#"export GH_TOKEN='a'\''b; rm -rf ~ #'"#));
        // The dangerous substring never appears unquoted at a statement start.
        assert!(!preamble.contains("\nrm -rf"));
    }

    #[test]
    fn sh_quote_escapes_embedded_single_quotes() {
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("a'b"), r#"'a'\''b'"#);
    }

    #[test]
    fn ssh_args_bound_the_connection() {
        let args = ssh_args("jack@mini-heart", None, false);
        // Primary hang killer: never block on an interactive prompt.
        assert!(args.iter().any(|a| a == "BatchMode=yes"));
        // Connect handshake and stalled-session bounds.
        assert!(args.iter().any(|a| a == "ConnectTimeout=10"));
        assert!(args.iter().any(|a| a == "ServerAliveInterval=10"));
        assert!(args.iter().any(|a| a == "ServerAliveCountMax=3"));
        // Still targets the destination and runs the piped preamble.
        assert!(args.iter().any(|a| a == "jack@mini-heart"));
        assert_eq!(args.last().unwrap(), "bash -s");
        // Agent forwarding stays opt-out; no -p without an explicit port.
        assert!(!args.iter().any(|a| a == "-A"));
        assert!(!args.iter().any(|a| a == "-p"));
    }

    #[test]
    fn ssh_args_pass_an_explicit_port() {
        let args = ssh_args("jack@host", Some(2222), false);
        let p = args.iter().position(|a| a == "-p").expect("-p present");
        assert_eq!(args[p + 1], "2222");
    }

    #[test]
    fn ssh_args_opt_in_agent_forwarding() {
        let args = ssh_args("host", None, true);
        assert_eq!(args.first().unwrap(), "-A");
    }

    #[test]
    fn classify_exit_separates_transport_from_command_failure() {
        assert_eq!(classify_exit(Some(0)), SshOutcome::Success);
        // ssh's reserved transport code and death-by-signal are connection phase.
        assert_eq!(classify_exit(Some(255)), SshOutcome::ConnectionFailure);
        assert_eq!(classify_exit(None), SshOutcome::ConnectionFailure);
        // A real remote command code is propagated, not swallowed as transport.
        assert_eq!(classify_exit(Some(1)), SshOutcome::CommandFailure(1));
        assert_eq!(classify_exit(Some(42)), SshOutcome::CommandFailure(42));
    }

    #[test]
    fn connection_error_names_host_without_leaking_credentials() {
        let err = connection_error("mini-heart").to_string();
        assert!(err.contains("mini-heart"));
        assert!(err.contains("connection/transport"));
        // Nothing credential-shaped in the sanitized message.
        assert!(!err.contains("TOKEN"));
        assert!(!err.contains("password"));
    }

    #[test]
    fn env_name_validation_rejects_injection() {
        assert!(is_valid_env_name("STRIPE_KEY"));
        assert!(is_valid_env_name("_x1"));
        assert!(!is_valid_env_name("1BAD"));
        assert!(!is_valid_env_name("A B"));
        assert!(!is_valid_env_name("A=B; rm -rf ~"));
        assert!(!is_valid_env_name(""));
    }
}
