//! `lf ssh <HomeId|host> <lf-args...>` — run `lf` on a remote machine.
//!
//! Foreground commands bring narrowly resolved local credentials. Managed
//! Claude/Codex accounts stay behind a foreground Unix-socket broker; the
//! remote receives only an opaque lease handle and a provider process receives
//! only its selected token. Durable work sheds all forwarded authority before
//! it detaches and uses credentials installed on the target machine.
//!
//! A `HomeId` target resolves through the locally observed route and makes the
//! remote process prove that it is the addressed Home before dispatch.
//!
//! Forwarded authority: GitHub (`gh`), Claude/Codex agent OAuth, and — the
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
use clap::Parser;

use crate::durable::HomeId;
use crate::pm::PmProviderKind;
use crate::provider_account::lease::{
    self, AccountLeaseBroker, AccountLeaseHandle, AccountSelection, PreparedAccountLease,
};
use crate::provider_auth::{
    extract_claude_token, extract_codex_access_token, extract_opencode_zen_token,
};

pub const EXPECTED_HOME_ID_ENV: &str = "LF_EXPECTED_HOME_ID";

/// Default repository path (relative to `$HOME`) the remote command runs in.
pub const DEFAULT_REPO: &str = "src/loopflow";

/// The local credential bundle forwarded to the remote. Absent credentials are
/// simply not exported — the remote falls back to whatever it can resolve.
#[derive(Default)]
struct Credentials {
    gh_token: Option<String>,
    provider_authority: ProviderAuthority,
    opencode_token: Option<String>,
    pm_token: Option<String>,
    /// PM provider the token belongs to (e.g. `linear`).
    pm_provider: Option<String>,
    /// Doppler-backed secrets resolved locally, forwarded as `export NAME=value`.
    secrets: Vec<(String, String)>,
}

enum ProviderAuthority {
    Ambient {
        claude_token: Option<String>,
        codex_token: Option<String>,
    },
    Lease(PreparedAccountLease),
}

impl Default for ProviderAuthority {
    fn default() -> Self {
        Self::Ambient {
            claude_token: None,
            codex_token: None,
        }
    }
}

impl Credentials {
    fn take_account_lease(&mut self) -> Option<PreparedAccountLease> {
        match std::mem::take(&mut self.provider_authority) {
            ProviderAuthority::Lease(lease) => Some(lease),
            ambient @ ProviderAuthority::Ambient { .. } => {
                self.provider_authority = ambient;
                None
            }
        }
    }
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let provider_authority = match &self.provider_authority {
            ProviderAuthority::Ambient {
                claude_token,
                codex_token,
            } => format!(
                "ambient(claude={}, codex={})",
                claude_token.is_some(),
                codex_token.is_some()
            ),
            ProviderAuthority::Lease(_) => "lease".to_string(),
        };
        formatter
            .debug_struct("Credentials")
            .field("gh_token", &self.gh_token.is_some())
            .field("provider_authority", &provider_authority)
            .field("opencode_token", &self.opencode_token.is_some())
            .field("pm_token", &self.pm_token.is_some())
            .field("pm_provider", &self.pm_provider)
            .field(
                "secrets",
                &self
                    .secrets
                    .iter()
                    .map(|(name, _)| name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Run `lf` on `host` in `$HOME/<repo>` with the local credential bundle
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
    target: &str,
    repo: Option<&str>,
    secret_names: &[String],
    forward_agent: bool,
    selection: &AccountSelection,
    lf_args: &[String],
) -> anyhow::Result<()> {
    reject_nested_ssh(lf_args)?;
    let target = resolve_target(target)?;
    let lf_args = bind_home_start_wave_ids(&target, lf_args)?;
    let cmd = std::iter::once("lf".to_string())
        .chain(lf_args)
        .collect::<Vec<_>>();
    let mut extra_env = vec![(crate::engine::machine::SSH_TARGET_ENV, target.dest.as_str())];
    if let Some(home_id) = target.home_id.as_ref().map(HomeId::as_str) {
        extra_env.push((EXPECTED_HOME_ID_ENV, home_id));
    }
    run_with_env(
        &target.dest,
        target.port,
        repo,
        secret_names,
        forward_agent,
        selection,
        &cmd,
        &extra_env,
    )
}

fn bind_home_start_wave_ids(target: &SshTarget, lf_args: &[String]) -> anyhow::Result<Vec<String>> {
    if target.home_id.is_none() {
        return Ok(lf_args.to_vec());
    }
    let parsed = crate::lf::Cli::try_parse_from(
        std::iter::once("lf".to_string()).chain(lf_args.iter().cloned()),
    );
    let Ok(crate::lf::Cli {
        command: Some(crate::lf::Commands::Start {
            waves, wave_ids, ..
        }),
        ..
    }) = parsed
    else {
        return Ok(lf_args.to_vec());
    };
    if waves.is_empty() {
        return Ok(lf_args.to_vec());
    }
    let existing = wave_ids
        .iter()
        .filter_map(|binding| binding.split_once('=').map(|(name, _)| name.to_string()))
        .collect::<std::collections::HashSet<_>>();
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let bindings = runtime.block_on(async {
        let Some(store) = crate::store::open_existing_store().await else {
            return Ok::<_, anyhow::Error>(Vec::new());
        };
        let mut bindings = Vec::new();
        for raw_name in waves {
            let Some(name) = crate::ops::util::normalize_wave_name(&raw_name) else {
                continue;
            };
            if existing.contains(&name) {
                continue;
            }
            if let Some(wave) = store.get_wave_by_name(&name).await? {
                bindings.push(format!("{}={}", wave.name(), wave.id()));
            }
        }
        Ok(bindings)
    })?;
    let mut bound = lf_args.to_vec();
    for binding in bindings {
        bound.push("--wave-id".to_string());
        bound.push(binding);
    }
    Ok(bound)
}

fn reject_nested_ssh(lf_args: &[String]) -> anyhow::Result<()> {
    if lf_args.first().is_some_and(|arg| arg == "lf") {
        return Err(anyhow!(
            "the remote `lf` is implicit; use `lf ssh <target> <args...>` without `-- lf`"
        ));
    }
    let args = std::iter::once("lf".to_string())
        .chain(lf_args.iter().cloned())
        .collect::<Vec<_>>();
    if matches!(
        crate::lf::Cli::try_parse_from(args),
        Ok(crate::lf::Cli {
            command: Some(crate::lf::Commands::Ssh { .. }),
            ..
        })
    ) {
        return Err(anyhow!(
            "nested `lf ssh` is not supported; connect directly from the origin machine"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SshTarget {
    dest: String,
    port: Option<u16>,
    home_id: Option<HomeId>,
}

fn resolve_target(target: &str) -> anyhow::Result<SshTarget> {
    let Ok(home_id) = HomeId::parse(target) else {
        return Ok(SshTarget {
            dest: target.to_string(),
            port: None,
            home_id: None,
        });
    };
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let home = runtime
        .block_on(async {
            let store = crate::store::open_existing_store().await?;
            store.home_by_id(&home_id).await.ok().flatten()
        })
        .ok_or_else(|| anyhow!("Home {home_id} was not found in the local store"))?;
    let route = crate::engine::wave_home::HomeRoute::parse(&home.route).ok_or_else(|| {
        anyhow!(
            "Home {home_id} route {:?} is not a remote SSH route",
            home.route
        )
    })?;
    Ok(SshTarget {
        dest: route
            .ssh_destination()
            .ok_or_else(|| anyhow!("Home {home_id} is local; lf ssh needs a remote Home"))?,
        port: route.ssh_port(),
        home_id: Some(home_id),
    })
}

pub fn capture_home_command(
    home_id: &HomeId,
    repo: &str,
    cmd: &[String],
) -> Result<String, SshCaptureError> {
    let target = resolve_target(home_id.as_str())
        .map_err(|error| SshCaptureError::Local(error.to_string()))?;
    let preamble = build_preamble(
        &Credentials::default(),
        None,
        &target.dest,
        repo,
        cmd,
        &[(EXPECTED_HOME_ID_ENV, home_id.as_str())],
    );
    run_ssh_capture(&target.dest, target.port, None, &preamble)
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

impl std::fmt::Display for SshCaptureError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unreachable(reason) | Self::Local(reason) => formatter.write_str(reason),
            Self::Command { code, stderr } if stderr.is_empty() => {
                write!(formatter, "remote command exited with status {code}")
            }
            Self::Command { code, stderr } => {
                write!(
                    formatter,
                    "remote command exited with status {code}: {stderr}"
                )
            }
        }
    }
}

impl std::error::Error for SshCaptureError {}

#[allow(clippy::too_many_arguments)]
fn run_with_env(
    dest: &str,
    port: Option<u16>,
    repo: Option<&str>,
    secret_names: &[String],
    forward_agent: bool,
    selection: &AccountSelection,
    cmd: &[String],
    extra_env: &[(&str, &str)],
) -> anyhow::Result<()> {
    if lease::account_lease_active() {
        return Err(anyhow!(
            "an inherited account lease cannot be re-forwarded over SSH; put `lf ssh` on the outer account-selected invocation"
        ));
    }
    let repo = repo.unwrap_or(DEFAULT_REPO);
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let mut credentials = runtime.block_on(resolve_credentials(secret_names, selection))?;
    if let ProviderAuthority::Lease(prepared) = &credentials.provider_authority {
        println!("Account lease: {}", format_account_plan(&prepared.lease));
    }
    let account_lease = credentials.take_account_lease();
    reject_detached_account_forwarding(account_lease.is_some(), cmd)?;
    let broker = account_lease.map(AccountLeaseBroker::start).transpose()?;
    let remote_handle = broker.as_ref().map(AccountLeaseBroker::remote_handle);
    let preamble = build_preamble(
        &credentials,
        remote_handle.as_ref(),
        dest,
        repo,
        cmd,
        extra_env,
    );
    let outcome = run_ssh(dest, port, forward_agent, broker.as_ref(), &preamble)?;
    // `process::exit` skips destructors. Close the broker and remove its local
    // socket before preserving a nonzero remote command's exact exit code.
    drop(broker);
    match outcome {
        SshOutcome::Success => Ok(()),
        SshOutcome::CommandFailure(code) => std::process::exit(code),
        SshOutcome::ConnectionFailure => {
            unreachable!("run_ssh returns transport failures as errors")
        }
    }
}

fn format_account_plan(lease: &lease::AccountLease) -> String {
    lease
        .grants
        .iter()
        .map(|grant| {
            let accounts = grant
                .accounts
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>();
            let route = match accounts.as_slice() {
                [] => "unavailable".to_string(),
                [account] => account.clone(),
                [first, rest @ ..] => format!("{first}, then {}", rest.join(", then ")),
            };
            format!("{}: {route}", grant.provider.display_name())
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn reject_detached_account_forwarding(
    has_account_lease: bool,
    cmd: &[String],
) -> anyhow::Result<()> {
    let remote_lf_command = cmd
        .first()
        .is_some_and(|program| program == "lf")
        .then(|| cmd.get(1))
        .flatten();
    let detached_program = remote_lf_command.is_some_and(|program| {
        matches!(
            program.as_str(),
            "tmux" | "screen" | "nohup" | "daemon" | "systemd-run"
        )
    });
    if has_account_lease && (detached_program || cmd.iter().any(|arg| arg == "--detach")) {
        return Err(anyhow!(
            "cannot forward ephemeral provider accounts into detached remote work; \
             run the remote command in the foreground or authenticate on the remote host"
        ));
    }
    Ok(())
}

/// Resolve the credential bundle from local sources. Auth tokens that aren't
/// present resolve to `None`; a `--secret` that can't be resolved is a hard
/// error (the caller explicitly asked for it). Nothing here prints a value.
async fn resolve_credentials(
    secret_names: &[String],
    selection: &AccountSelection,
) -> anyhow::Result<Credentials> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut secrets = Vec::with_capacity(secret_names.len());
    for name in secret_names {
        secrets.push((name.clone(), resolve_doppler_secret(name)?));
    }
    let account_lease = lease::prepare_root_lease(selection).await?;
    let provider_authority = match account_lease {
        Some(lease) => ProviderAuthority::Lease(lease),
        None => ProviderAuthority::Ambient {
            claude_token: extract_claude_token(&home).map(|token| token.access_token),
            codex_token: extract_codex_access_token(&home),
        },
    };
    Ok(Credentials {
        gh_token: resolve_gh_token(),
        provider_authority,
        opencode_token: _resolve_opencode_token(&home).await,
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
    _resolve_stored_provider_token(PmProviderKind::Linear.as_str()).await
}

async fn _resolve_opencode_token(home: &std::path::Path) -> Option<String> {
    _resolve_stored_provider_token("opencodezen")
        .await
        .or_else(|| extract_opencode_zen_token(home).map(|token| token.access_token))
}

async fn _resolve_stored_provider_token(provider: &str) -> Option<String> {
    let cfg = crate::store::storage_config_from_env().ok()?;
    let store = crate::store::open_store(&cfg).await.ok()?;
    let token = store.get_provider_token(provider).await.ok()??;
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
    lease_handle: Option<&AccountLeaseHandle>,
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

    // Transport-supplied identity markers are exported before credentials so
    // the remote can verify them during its earliest dispatch checks.
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
    if let ProviderAuthority::Ambient {
        claude_token,
        codex_token,
    } = &credentials.provider_authority
    {
        if let Some(token) = nonempty(claude_token) {
            lines.push(format!(
                "export CLAUDE_CODE_OAUTH_TOKEN={}",
                sh_quote(token)
            ));
        }
        if let Some(token) = nonempty(codex_token) {
            lines.push(format!("export CODEX_ACCESS_TOKEN={}", sh_quote(token)));
        }
    }
    if let Some(token) = nonempty(&credentials.opencode_token) {
        lines.push(format!("export OPENCODE_API_KEY={}", sh_quote(token)));
    }
    if let Some(lease_handle) = lease_handle {
        match lease_handle.encode() {
            Ok(handle) => lines.push(format!(
                "export {}={}",
                lease::ACCOUNT_LEASE_ENV,
                sh_quote(&handle)
            )),
            Err(error) => lines.push(format!(
                "echo {} >&2; exit 1",
                sh_quote(&format!("could not encode account lease: {error}"))
            )),
        }
        lines.push("unset CLAUDE_CODE_OAUTH_TOKEN ANTHROPIC_API_KEY CLAUDE_CONFIG_DIR".to_string());
        lines.push("unset CODEX_ACCESS_TOKEN OPENAI_API_KEY CODEX_HOME".to_string());
        lines.push(format!(
            "LF_ACCOUNT_LEASE_SOCKET={}",
            sh_quote(&lease_handle.socket.to_string_lossy())
        ));
        lines.push(
            "trap 'status=$?; trap - EXIT; rm -f -- \"$LF_ACCOUNT_LEASE_SOCKET\"; exit \"$status\"' EXIT"
                .to_string(),
        );
        lines.push("trap 'exit 129' HUP".to_string());
        lines.push("trap 'exit 130' INT".to_string());
        lines.push("trap 'exit 143' TERM".to_string());
        lines.push(
            "lf --__account-lease-probe || { echo 'remote lf cannot use the forwarded account lease; install the same loopflow version on both hosts' >&2; exit 1; }"
                .to_string(),
        );
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
    if !credentials.secrets.is_empty() {
        lines.push(format!(
            "export LF_FORWARDED_SECRET_NAMES={}",
            sh_quote(
                &credentials
                    .secrets
                    .iter()
                    .map(|(name, _)| name.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        ));
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
    // Under a lease the shell must survive the command so its EXIT trap can
    // remove the forwarded socket; without one, exec saves a process.
    lines.push(if lease_handle.is_some() {
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
fn ssh_args(
    dest: &str,
    port: Option<u16>,
    forward_agent: bool,
    broker: Option<&AccountLeaseBroker>,
) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    if forward_agent {
        args.push("-A".to_string());
    }
    if let Some(port) = port {
        args.push("-p".to_string());
        args.push(port.to_string());
    }
    if let Some(broker) = broker {
        args.push("-R".to_string());
        args.push(format!(
            "{}:{}",
            broker.remote_socket().display(),
            broker.local_socket().display()
        ));
        args.push("-o".to_string());
        args.push("StreamLocalBindUnlink=yes".to_string());
        args.push("-o".to_string());
        args.push("StreamLocalBindMask=0177".to_string());
        args.push("-o".to_string());
        args.push("ExitOnForwardFailure=yes".to_string());
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
/// classifying the remote exit code. Agent forwarding (`-A`) is opt-in. Bounded
/// so an unreachable or misconfigured host fails fast instead of hanging.
fn run_ssh(
    dest: &str,
    port: Option<u16>,
    forward_agent: bool,
    broker: Option<&AccountLeaseBroker>,
    preamble: &str,
) -> anyhow::Result<SshOutcome> {
    let mut child = Command::new("ssh")
        .args(ssh_args(dest, port, forward_agent, broker))
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
        outcome @ (SshOutcome::Success | SshOutcome::CommandFailure(_)) => Ok(outcome),
        SshOutcome::ConnectionFailure => Err(connection_error(dest)),
    }
}

/// Like [`run_ssh`] but captures stdout (the preamble on stdin, stdout to a
/// buffer, stderr still inherited so ssh's own diagnostics stay visible).
/// Classifies transport failure vs. a nonzero remote command so the Home probe
/// can tell "unreachable" from "answered oddly".
fn run_ssh_capture(
    dest: &str,
    port: Option<u16>,
    broker: Option<&AccountLeaseBroker>,
    preamble: &str,
) -> Result<String, SshCaptureError> {
    let mut child = Command::new("ssh")
        .args(ssh_args(dest, port, false, broker))
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
    use super::*;

    fn full_credentials() -> Credentials {
        Credentials {
            gh_token: Some("gh-secret".to_string()),
            provider_authority: ProviderAuthority::default(),
            opencode_token: Some("opencode-secret".to_string()),
            pm_token: Some("linear-secret".to_string()),
            pm_provider: Some("linear".to_string()),
            secrets: vec![("STRIPE_KEY".to_string(), "sk-live-123".to_string())],
        }
    }

    fn lease_handle() -> AccountLeaseHandle {
        AccountLeaseHandle {
            socket: PathBuf::from("/tmp/lf-account-test.sock"),
            secret: "test-secret".to_string(),
        }
    }

    #[test]
    fn remote_tmux_rejects_ephemeral_account_forwarding() {
        let cmd = vec![
            "lf".to_string(),
            "tmux".to_string(),
            "list-sessions".to_string(),
        ];

        assert!(reject_detached_account_forwarding(false, &cmd).is_ok());
        assert!(reject_detached_account_forwarding(true, &cmd)
            .unwrap_err()
            .to_string()
            .contains("detached remote work"));
        assert!(reject_detached_account_forwarding(
            true,
            &["lf".to_string(), "wave".to_string(), "--detach".to_string()]
        )
        .is_err());
    }

    #[test]
    fn nested_ssh_is_rejected_before_transport() {
        let _lock = crate::journal::test_env_lock();
        let previous = std::env::var_os(lease::ACCOUNT_LEASE_ENV);
        std::env::set_var(lease::ACCOUNT_LEASE_ENV, "forwarded");
        let result = run(
            "must-not-be-reached.invalid",
            None,
            &[],
            false,
            &AccountSelection::default(),
            &[
                "--account".to_string(),
                "forwarded@example.com".to_string(),
                "ssh".to_string(),
                "second-hop".to_string(),
                "task".to_string(),
                "pursue".to_string(),
            ],
        );
        match previous {
            Some(value) => std::env::set_var(lease::ACCOUNT_LEASE_ENV, value),
            None => std::env::remove_var(lease::ACCOUNT_LEASE_ENV),
        }
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("nested `lf ssh` is not supported"));
    }

    #[test]
    fn preamble_exports_every_credential_and_execs_command() {
        let cmd = vec!["lf".to_string(), "op".to_string(), "pr".to_string()];
        let handle = lease_handle();
        let preamble = build_preamble(
            &full_credentials(),
            Some(&handle),
            "mini-heart",
            "src/loopflow",
            &cmd,
            &[(crate::engine::machine::SSH_TARGET_ENV, "mini-heart")],
        );

        assert!(preamble.contains("export GH_TOKEN='gh-secret'"));
        assert!(preamble.contains("export LF_SSH_TARGET="));
        assert!(preamble.contains("export OPENCODE_API_KEY='opencode-secret'"));
        assert!(!preamble.contains("export CLAUDE_CODE_OAUTH_TOKEN="));
        assert!(!preamble.contains("export CODEX_ACCESS_TOKEN="));
        assert!(preamble.contains(&format!("export {}=", lease::ACCOUNT_LEASE_ENV)));
        assert!(!preamble.contains("claude-primary"));
        assert!(!preamble.contains("codex-reserve"));
        assert!(!preamble.contains("refresh_token"));
        assert!(!preamble.contains("/accounts/"));
        assert!(preamble.contains("lf --__account-lease-probe"));
        assert!(preamble.contains("trap - EXIT; rm -f --"));
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

    #[test]
    fn preamble_omits_absent_credentials() {
        let creds = Credentials {
            provider_authority: ProviderAuthority::Ambient {
                claude_token: Some("only-claude".to_string()),
                codex_token: None,
            },
            ..Credentials::default()
        };
        let cmd = vec!["lf".to_string(), "runs".to_string()];
        let preamble = build_preamble(&creds, None, "host", "src/loopflow", &cmd, &[]);

        assert!(preamble.contains("export CLAUDE_CODE_OAUTH_TOKEN='only-claude'"));
        assert!(!preamble.contains("GH_TOKEN"));
        assert!(!preamble.contains("LF_FORWARDED_PM_TOKEN"));
        assert!(!preamble.contains("GIT_CONFIG_COUNT"));
        assert!(!preamble.contains("credential.helper"));
    }

    #[test]
    fn home_command_carries_identity_and_no_origin_authority() {
        let cmd = vec!["lf".to_string(), "start".to_string(), "product".to_string()];
        let preamble = build_preamble(
            &Credentials::default(),
            None,
            "jack@buildbox",
            "src/loopflow",
            &cmd,
            &[(
                EXPECTED_HOME_ID_ENV,
                "home_00000000000000000000000000000001",
            )],
        );

        assert!(
            preamble.contains("export LF_EXPECTED_HOME_ID='home_00000000000000000000000000000001'")
        );
        for secret in [
            "GH_TOKEN",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CODEX_ACCESS_TOKEN",
            "OPENCODE_API_KEY",
            "LF_FORWARDED_PM_TOKEN",
            lease::ACCOUNT_LEASE_ENV,
        ] {
            assert!(!preamble.contains(secret));
        }
        assert!(preamble.trim_end().ends_with("exec 'lf' 'start' 'product'"));
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
        let preamble = build_preamble(&creds, None, "host", "src/loopflow", &cmd, &[]);

        assert!(preamble.contains(r#"export GH_TOKEN='a'\''b; rm -rf ~ #'"#));
        // The dangerous substring never appears unquoted at a statement start.
        assert!(!preamble.contains("\nrm -rf"));
    }

    #[test]
    fn credential_debug_output_redacts_values() {
        let debug = format!("{:?}", full_credentials());

        assert!(!debug.contains("gh-secret"));
        assert!(!debug.contains("linear-secret"));
        assert!(!debug.contains("opencode-secret"));
        assert!(!debug.contains("sk-live-123"));
        assert!(debug.contains("STRIPE_KEY"));
    }

    #[test]
    fn sh_quote_escapes_embedded_single_quotes() {
        assert_eq!(sh_quote("plain"), "'plain'");
        assert_eq!(sh_quote("a'b"), r#"'a'\''b'"#);
    }

    #[test]
    fn ssh_args_bound_the_connection() {
        let args = ssh_args("jack@mini-heart", None, false, None);
        // Primary hang killer: never block on an interactive prompt.
        assert!(args.iter().any(|a| a == "BatchMode=yes"));
        // Connect handshake and stalled-session bounds.
        assert!(args.iter().any(|a| a == "ConnectTimeout=10"));
        assert!(args.iter().any(|a| a == "ServerAliveInterval=10"));
        assert!(args.iter().any(|a| a == "ServerAliveCountMax=3"));
        // Still targets the destination and runs the piped preamble.
        assert!(args.iter().any(|a| a == "jack@mini-heart"));
        assert_eq!(args.last().unwrap(), "bash -s");
        assert_eq!(
            args.iter().filter(|arg| arg.as_str() == "bash -s").count(),
            1
        );
        // Agent forwarding stays opt-out; no -p without an explicit port.
        assert!(!args.iter().any(|a| a == "-A"));
        assert!(!args.iter().any(|a| a == "-p"));
    }

    #[test]
    fn ssh_args_pass_an_explicit_port() {
        let args = ssh_args("jack@host", Some(2222), false, None);
        let p = args.iter().position(|a| a == "-p").expect("-p present");
        assert_eq!(args[p + 1], "2222");
    }

    #[test]
    fn ssh_args_opt_in_agent_forwarding() {
        let args = ssh_args("host", None, true, None);
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
