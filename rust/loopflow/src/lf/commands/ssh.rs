//! `lf ssh <host> -- <cmd...>` — run a command on a remote home carrying the
//! caller's *local* credentials, resolved per invocation.
//!
//! The "bring your auth with you" model: credentials are resolved on this
//! machine, forwarded into the remote process environment as a stdin preamble
//! (never in argv, `ps`, or logs), and are NOT persisted on the remote — they
//! die with the process. The remote host stays a stateless compute surface.
//!
//! Forwarded bundle: GitHub (`gh`), Claude/agent OAuth, and — the capability
//! beyond the shell prototype — the PM/Linear token, which lives in lfdb rather
//! than the environment. The remote `resolve_pm_token` reads
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

use crate::lfd::pm::PmProviderKind;
use crate::provider_auth::extract_claude_token;

/// Default repository path (relative to `$HOME`) the remote command runs in.
pub const DEFAULT_REPO: &str = "src/loopflow";

/// The local credential bundle forwarded to the remote. Absent credentials are
/// simply not exported — the remote falls back to whatever it can resolve.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Credentials {
    pub gh_token: Option<String>,
    pub claude_token: Option<String>,
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
    host: &str,
    repo: Option<&str>,
    secret_names: &[String],
    forward_agent: bool,
    cmd: &[String],
) -> anyhow::Result<()> {
    if cmd.is_empty() {
        return Err(anyhow!(
            "lf ssh needs a command after `--`, e.g. `lf ssh {host} -- lf pr open`"
        ));
    }
    let repo = repo.unwrap_or(DEFAULT_REPO);
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let credentials = runtime.block_on(resolve_credentials(secret_names))?;
    let preamble = build_preamble(&credentials, host, repo, cmd);
    run_ssh(host, forward_agent, &preamble)
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
    Ok(Credentials {
        gh_token: resolve_gh_token(),
        claude_token: extract_claude_token(&home).map(|token| token.access_token),
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

/// PM/Linear access token from the local lfdb credential store. Absent when no
/// store exists or no Linear credential is stored.
async fn resolve_pm_token() -> Option<String> {
    let cfg = crate::lfd::storage_config_from_env().ok()?;
    let store = crate::lfdb::open_store(&cfg).await.ok()?;
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
fn build_preamble(credentials: &Credentials, host: &str, repo: &str, cmd: &[String]) -> String {
    let mut lines: Vec<String> = Vec::new();

    // Give the remote a sane PATH so `lf`, `gh` resolve under a
    // non-interactive `bash -s`.
    lines.push(
        "export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\""
            .to_string(),
    );

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
    lines.push(format!("exec {remote_cmd}"));

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

/// Pipe the preamble into `ssh [-A] <host> bash -s`, streaming stdout/stderr and
/// propagating the remote exit code. Agent forwarding (`-A`) is opt-in.
fn run_ssh(host: &str, forward_agent: bool, preamble: &str) -> anyhow::Result<()> {
    let mut command = Command::new("ssh");
    if forward_agent {
        command.arg("-A");
    }
    let mut child = command
        .args([host, "bash -s"])
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
    if status.success() {
        Ok(())
    } else {
        std::process::exit(status.code().unwrap_or(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_bundle() -> Credentials {
        Credentials {
            gh_token: Some("gh-secret".to_string()),
            claude_token: Some("claude-secret".to_string()),
            pm_token: Some("linear-secret".to_string()),
            pm_provider: Some("linear".to_string()),
            secrets: vec![("STRIPE_KEY".to_string(), "sk-live-123".to_string())],
        }
    }

    #[test]
    fn preamble_exports_every_credential_and_execs_command() {
        let cmd = vec!["lf".to_string(), "op".to_string(), "pr".to_string()];
        let preamble = build_preamble(&full_bundle(), "mini-heart", "src/loopflow", &cmd);

        assert!(preamble.contains("export GH_TOKEN='gh-secret'"));
        assert!(preamble.contains("export CLAUDE_CODE_OAUTH_TOKEN='claude-secret'"));
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
        // cd into the repo and exec the command
        assert!(preamble.contains("cd \"$HOME\"/'src/loopflow'"));
        assert!(preamble.trim_end().ends_with("exec 'lf' 'op' 'pr'"));
    }

    #[test]
    fn preamble_omits_absent_credentials() {
        let creds = Credentials {
            claude_token: Some("only-claude".to_string()),
            ..Credentials::default()
        };
        let cmd = vec!["lf".to_string(), "runs".to_string()];
        let preamble = build_preamble(&creds, "host", "src/loopflow", &cmd);

        assert!(preamble.contains("export CLAUDE_CODE_OAUTH_TOKEN='only-claude'"));
        assert!(!preamble.contains("GH_TOKEN"));
        assert!(!preamble.contains("LF_FORWARDED_PM_TOKEN"));
        assert!(!preamble.contains("GIT_CONFIG_COUNT"));
        assert!(!preamble.contains("credential.helper"));
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
        let preamble = build_preamble(&creds, "host", "src/loopflow", &cmd);

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
    fn env_name_validation_rejects_injection() {
        assert!(is_valid_env_name("STRIPE_KEY"));
        assert!(is_valid_env_name("_x1"));
        assert!(!is_valid_env_name("1BAD"));
        assert!(!is_valid_env_name("A B"));
        assert!(!is_valid_env_name("A=B; rm -rf ~"));
        assert!(!is_valid_env_name(""));
    }
}
