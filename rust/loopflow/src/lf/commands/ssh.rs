//! `lf ssh <label> <lf-args...>` — run `lf` on a remote machine.
//!
//! Foreground commands bring narrowly resolved local credentials. Managed
//! Claude/Codex accounts stay behind a foreground Unix-socket broker; the
//! remote receives only an opaque lease handle and a provider process receives
//! only its selected token. Durable work sheds all forwarded authority before
//! it detaches and uses credentials installed on the target machine.
//!
//! An added label or `MachineId` resolves through its saved route and makes the
//! remote process prove that it is the addressed Machine before dispatch.
//!
//! Forwarded authority: GitHub (`gh`), Claude/Codex agent OAuth, and — the
//! capability beyond the shell prototype — the PM/Linear token, which lives in
//! store rather than the environment. The remote `resolve_pm_token` reads
//! `LF_FORWARDED_PM_TOKEN` before its (empty) store, so remote `lf repo refresh` works.
//!
//! Secrets policy: `lf machine ssh` forwards specific resolved secrets, never the
//! Doppler token that could fetch them all. The Doppler login/CLI token is a
//! master key to the whole secret estate and never leaves this machine. When a
//! remote command needs a Doppler-backed secret, name it with `--secret NAME`:
//! the value is resolved *locally* via Doppler and only that value is forwarded.
//! Agent forwarding (`ssh -A`) is off by default — git pushes ride the
//! forwarded `GH_TOKEN` over HTTPS, so the caller's SSH identity stays home.

use clap::Parser;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Context};

use crate::pm::PmProviderKind;
use crate::provider_account::lease::{
    self, AccountLeaseBroker, AccountLeaseHandle, AccountSelection, PreparedAccountLease,
};
use crate::provider_auth::{
    extract_claude_token, extract_codex_access_token, extract_opencode_zen_token,
};

pub const EXPECTED_MACHINE_ID_ENV: &str = "LF_EXPECTED_MACHINE_ID";

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

/// Run `lf` on an added machine in its repository with the local credential bundle
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
    let runtime = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    let target = runtime.block_on(resolve_target(target, forward_agent))?;
    let cmd = std::iter::once("lf".to_string())
        .chain(lf_args.iter().cloned())
        .collect::<Vec<_>>();
    if lease::account_lease_active() {
        return Err(anyhow!(
            "an inherited account lease cannot be re-forwarded over SSH; put `lf machine ssh` on the outer account-selected invocation"
        ));
    }
    let mut credentials = runtime.block_on(resolve_credentials(secret_names, selection))?;
    if let ProviderAuthority::Lease(prepared) = &credentials.provider_authority {
        println!("Account lease: {}", format_account_plan(&prepared.lease));
    }
    let account_lease = credentials.take_account_lease();
    reject_detached_account_forwarding(account_lease.is_some(), &cmd)?;
    let broker = account_lease.map(AccountLeaseBroker::start).transpose()?;
    let remote_handle = broker.as_ref().map(AccountLeaseBroker::remote_handle);
    let user_name = crate::engine::config::participant_name()?.unwrap_or_default();
    let declaration = std::env::var(crate::lf::WORK_DECLARATION_ENV).ok();
    let mut extra_env = vec![
        (EXPECTED_MACHINE_ID_ENV, target.id.as_str()),
        (crate::engine::config::USER_NAME_ENV, user_name.as_str()),
    ];
    if let Some(value) = declaration.as_deref() {
        extra_env.push((crate::lf::WORK_DECLARATION_ENV, value));
    }
    let preamble = build_preamble(
        &credentials,
        remote_handle.as_ref(),
        &target.route,
        repo.unwrap_or(
            target
                .repo
                .as_deref()
                .expect("added machines have a repository"),
        ),
        &cmd,
        &extra_env,
    );
    let outcome = run_ssh(&target.route, forward_agent, broker.as_ref(), &preamble)?;
    // Release the broker before reporting the remote command's result.
    drop(broker);
    match outcome {
        SshOutcome::Success => Ok(()),
        SshOutcome::CommandFailure(code) => Err(crate::process::CommandExit(
            u8::try_from(code).expect("SSH command exit status fits a byte"),
        )
        .into()),
        SshOutcome::ConnectionFailure => {
            unreachable!("run_ssh returns transport failures as errors")
        }
    }
}

fn reject_nested_ssh(lf_args: &[String]) -> anyhow::Result<()> {
    if lf_args.first().is_some_and(|arg| arg == "lf") {
        return Err(anyhow!(
            "the remote `lf` is implicit; use `lf machine ssh <target> <args...>` without `-- lf`"
        ));
    }
    let args = std::iter::once("lf".to_string())
        .chain(lf_args.iter().cloned())
        .collect::<Vec<_>>();
    if matches!(
        crate::lf::Cli::try_parse_from(crate::lf::navigation::normalize_args(args)?),
        Ok(crate::lf::Cli {
            command: Some(crate::lf::Commands::Machine {
                cmd: crate::lf::MachineCommand::Ssh { .. }
            }),
            ..
        })
    ) {
        return Err(anyhow!(
            "nested `lf machine ssh` is not supported; connect directly from the origin machine"
        ));
    }
    Ok(())
}

async fn resolve_target(
    target: &str,
    forward_agent: bool,
) -> anyhow::Result<crate::durable::Machine> {
    let store = crate::store::open_existing_store()
        .await
        .ok_or_else(|| anyhow!("machine commands need an initialized local store"))?;
    let machine = super::machine::find_machine(&store, target).await?;
    let probe = super::machine::probe(&machine.route, forward_agent).await?;
    super::machine::report_version(&machine.route, &probe.version);
    let reached = probe.id?;
    if reached != machine.id {
        return Err(anyhow!(
            "remote machine identity changed: expected {}, reached {reached}; remove and add the connection again",
            machine.id
        ));
    }
