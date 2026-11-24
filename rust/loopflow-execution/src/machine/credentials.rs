//! Foreground service credential transfer. Credential bytes travel only on stdin.
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::provider_auth::opencode_command as opencode;
use anyhow::{anyhow, bail, Context, Result};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::provider_auth::Provider;
use crate::store::{CredentialType, ProviderToken, SharedStore};
use loopflow_model::durable::Machine;

// Deliberately no Debug: native credential JSON and tokens must never be logged.
#[derive(Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
enum Credential {
    Github {
        login: String,
        token: String,
    },
    /// An access token the laptop just refreshed. Its refresh grant stays on the
    /// laptop, so no machine needs a browser approval or can rotate the grant away.
    Linear {
        login: String,
        access_token: String,
        expires_at: Option<i64>,
    },
    /// One `opencode auth export` entry; `login` is its OpenCode account, or the
    /// entry itself for an API key.
    OpenCode {
        login: String,
        entry: serde_json::Value,
    },
}

/// How long a refreshed Linear access token lasts (observed 2026-10-27). A
/// rental refreshed at prepare must not outlive it, or Linear stops working
/// there; raising `rental::MAX_HOURS` past it needs another way to renew.
const LINEAR_ACCESS_HOURS: f64 = 24.0;
const _: () = assert!(loopflow_storage::rental::MAX_HOURS <= LINEAR_ACCESS_HOURS);

pub(crate) async fn connect(
    machine: &Machine,
    provider: Provider,
    approver: Option<&crate::config::placement::Approver>,
    batch: bool,
) -> Result<()> {
    let store = crate::provider_auth::operations::local_store().await?;
    let mut verified = "";
    let credential = match provider {
        Provider::GitHub => {
            let token = gh_token()?;
            let login = github_login(token.expose_secret()).await?;
            if remote_has(machine, provider, &login).await? {
                return Ok(());
            }
            Credential::Github {
                login,
                token: token.expose_secret().clone(),
            }
        }
        Provider::Linear => {
            let current = store
                .get_provider_token("linear")
                .await?
                .ok_or_else(|| anyhow!("Linear is not connected on this laptop"))?;
            let login = linear_login(&current.access_token).await?;
            if remote_has(machine, provider, &login).await? {
                return Ok(());
            }
            let renewed = crate::ops::linear::renewed_planning_token(provider).await?;
            if linear_login(&renewed.access_token).await? != login {
                bail!("renewed Linear login belongs to a different account; discarded");
            }
            Credential::Linear {
                login,
                access_token: renewed.access_token,
                expires_at: renewed.expires_at,
            }
        }
        Provider::OpenCodeZen => {
            let entries = opencode_export(None, "laptop")?;
            let active = opencode_active(&entries).ok_or_else(|| {
                anyhow!("OpenCode has no active login on this laptop; run `opencode auth login`")
            })?;
            let login = opencode_identity(active)
                .ok_or_else(|| anyhow!("OpenCode's active login has no identity"))?
                .to_string();
            let oauth = active["value"]["type"] == "oauth";
            let account_email = active["value"]["metadata"]["email"].as_str();
            if !oauth {
                zen_check(opencode_token(active)).await?;
            }
            if remote_has(machine, provider, &login).await? {
                return Ok(());
            }
            require_foreground(batch)?;
            let entry = if oauth {
                // A copied login would share the laptop's refresh grant; mint a fresh one.
                connecting(machine, provider, account_email.unwrap_or(&login));
                let staging = tempfile::Builder::new()
                    .prefix("lf-machine-login-")
                    .tempdir()?;
                crate::provider_auth::operations::fresh_opencode_login(approver, staging.path())
                    .await?;
                let fresh = opencode_active(&opencode_export(Some(staging.path()), "laptop")?)
                    .cloned()
                    .ok_or_else(|| {
                        anyhow!("OpenCode device login completed without a credential")
                    })?;
                if opencode_identity(&fresh) != Some(login.as_str()) {
                    bail!("fresh OpenCode login belongs to a different account; discarded");
                }
                zen_check(opencode_token(&fresh)).await?;
                verified =
                    ": Zen accepted a fresh login for the same OpenCode account as this laptop";
                fresh
            } else {
                verified = ": Zen accepted the API key; its owner was not checked";
                active.clone()
            };
            Credential::OpenCode { login, entry }
        }
        _ => unreachable!("login::reaches_other_machines admits only these here"),
    };
    send(machine, &credential)?;
    println!(
        "{}: {} signed in{verified}",
        crate::machine::login::name(machine),
        provider.display_name()
    );
    Ok(())
}

fn send(machine: &Machine, credential: &Credential) -> Result<()> {
    let bytes = serde_json::to_vec(credential)?;
    remote_command(
        machine,
        &["machine", "credentials", "receive"],
        Some(&bytes),
    )?;
    Ok(())
}

fn gh_token() -> Result<SecretString> {
    let output = Command::new("gh")
        .args(["auth", "token", "--hostname", "github.com"])
        .stderr(Stdio::null())
        .output();
    match output {
        Ok(output) if output.status.success() => Ok(SecretString::new(
            String::from_utf8(output.stdout)?.trim().to_string(),
        )),
        _ => bail!("GitHub is not connected on this laptop; `gh auth login` first"),
    }
}

fn connecting(machine: &Machine, provider: Provider, login: &str) {
    eprintln!(
        "Connecting {} {login} on {}",
        provider.display_name(),
        crate::machine::login::name(machine)
    );
}

fn foreground(batch: bool) -> bool {
    use std::io::IsTerminal;
    // SAFETY: querying this process and stdin's foreground process group does not mutate either.
    let foreground = unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) == libc::getpgrp() };
    !batch && std::io::stdin().is_terminal() && foreground
}

pub(crate) fn require_foreground(batch: bool) -> Result<()> {
    if !foreground(batch) {
        bail!("a missing machine login needs a foreground terminal without --batch; run `lf machine login <provider> <machine>` there first");
    }
    Ok(())
}

/// Whether `machine` already reads signed in to `provider` as `login`; says so
/// when it does, since nothing is sent.
async fn remote_has(machine: &Machine, provider: Provider, login: &str) -> Result<bool> {
    let has = crate::machine::login::logins(machine)
        .await?
        .into_iter()
        .any(|row| {
            row.provider == provider
                && row.state == crate::machine::login::LoginState::SignedIn
                && row.account.as_deref() == Some(login)
        });
    if has {
        println!(
            "{}: {} already signed in as {login}",
            crate::machine::login::name(machine),
            provider.display_name()
        );
    }
    Ok(has)
}

pub(crate) fn remote_command(
    machine: &Machine,
    args: &[&str],
    input: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let script = format!(
        "{}; export {}={}; exec lf {}",
        crate::machine::ssh::remote_path(),
        crate::machine::ssh::EXPECTED_MACHINE_ID_ENV,
        crate::machine::ssh::sh_quote(machine.id.as_str()),
        args.iter()
            .map(|arg| crate::machine::ssh::sh_quote(arg))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let mut child = Command::new("ssh")
        .args(crate::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start machine credential transport")?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(input) = input {
            stdin.write_all(input)?;
        }
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        // Only the remote lf's own error line crosses: it never quotes credential
        // bytes, and its children's output is discarded there.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let reason = stderr
            .lines()
            // `Error:` from lf, `error:` from Clap when the remote lf is older.
            .find_map(|line| {
                line.strip_prefix("Error: ")
                    .or(line.strip_prefix("error: "))
            })
            .map(|line| format!(": {}", line.trim()))
            .unwrap_or_else(|| crate::os_process::exit_reason(&stderr));
        bail!(
            "machine credential command failed on {}{}",
            crate::machine::login::name(machine),
            if reason.is_empty() {
                format!(" ({})", output.status)
            } else {
                reason
            }
        );
    }
    Ok(output.stdout)
}

pub async fn receive(bytes: &[u8]) -> Result<()> {
    let store = crate::provider_auth::operations::local_store().await?;
    if bytes.len() > 1024 * 1024 {
        bail!("credential exceeds transfer size limit");
    }
    let credential: Credential =
        serde_json::from_slice(bytes).map_err(|_| anyhow!("invalid credential transfer"))?;
    install(&store, credential).await
}

async fn installed(store: &SharedStore, provider: Provider, login: &str) -> Result<bool> {
    match provider {
        Provider::GitHub => {
            let Some(config) = gh_hosts()? else {
                return Ok(false);
            };
            let github = &config["github.com"];
            Ok(github["users"][login]["oauth_token"]
                .as_str()
                .is_some_and(|token| !token.is_empty())
                || (github["user"].as_str() == Some(login)
                    && github["oauth_token"]
                        .as_str()
                        .is_some_and(|token| !token.is_empty())))
        }
        Provider::Linear => match store.get_provider_token("linear").await? {
            Some(token) => {
                let actual = linear_login(&token.access_token).await?;
                if actual != login {
                    bail!("a different Linear account is already installed; left unchanged");
                }
                Ok(true)
            }
            None => Ok(false),
        },
        Provider::OpenCodeZen => {
            let entries = opencode_export(None, "machine")?;
            let Some(active) = opencode_active(&entries) else {
                return Ok(false);
            };
            let accepted = zen_check(opencode_token(active)).await.is_ok();
            if opencode_identity(active) == Some(login) {
                return Ok(accepted);
            }
            if accepted {
                bail!(
                    "a different OpenCode login is already active on this machine; left unchanged"
                );
            }
            Ok(false)
        }
        _ => bail!("unsupported machine login provider"),
    }
}

/// gh's hosts file on this Machine, if it has one.
fn gh_hosts() -> Result<Option<serde_yaml_ng::Value>> {
    let directory = std::env::var_os("GH_CONFIG_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .expect("OS user has a home")
                .join(".config/gh")
        });
    let content = match fs::read_to_string(directory.join("hosts.yml")) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    serde_yaml_ng::from_str(&content)
        .map(Some)
        .map_err(|_| anyhow!("invalid GitHub credential file"))
}

/// The GitHub account gh is signed in as on this Machine.
pub(crate) fn github_user() -> Result<Option<String>> {
    Ok(gh_hosts()?.and_then(|config| config["github.com"]["user"].as_str().map(str::to_string)))
}

/// The OpenCode account active on this Machine, as a transfer names it; none
/// when OpenCode 2 is missing or has no active login.
pub(crate) fn opencode_login() -> Option<String> {
    let entries = opencode_export(None, "machine").ok()?;
    opencode_active(&entries)
        .and_then(opencode_identity)
        .map(str::to_string)
}

async fn install(store: &SharedStore, credential: Credential) -> Result<()> {
    match credential {
        Credential::Github { login, token } => {
            let _lock = credential_lock("github")?;
            if installed(store, Provider::GitHub, &login).await? {
                return Ok(());
            }
            if github_login(&token).await? != login {
                bail!("GitHub credential identity mismatch");
            }
            let mut child = Command::new("gh")
                .args([
                    "auth",
                    "login",
                    "--hostname",
                    "github.com",
                    "--git-protocol",
                    "https",
                    "--insecure-storage",
                    "--with-token",
                ])
                .env_remove("GH_TOKEN")
                .env_remove("GITHUB_TOKEN")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| match error.kind() {
                    std::io::ErrorKind::NotFound => anyhow!("GitHub CLI (gh) is not installed on this machine; install it (https://cli.github.com) and run `lf machine login github` again"),
                    _ => error.into(),
                })?;
            child
                .stdin
                .take()
                .expect("piped gh stdin")
                .write_all(token.as_bytes())?;
            if !child.wait()?.success() {
                bail!("GitHub credential installation failed");
            }
            if !installed(store, Provider::GitHub, &login).await? {
                bail!("GitHub did not retain the file-backed login");
            }
            let status = Command::new("gh")
                .args(["auth", "setup-git", "--hostname", "github.com"])
                .env_remove("GH_TOKEN")
                .env_remove("GITHUB_TOKEN")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()?;
            if !status.success() {
                bail!("GitHub login installed but Git credential setup failed");
            }
            Ok(())
        }
        Credential::Linear {
            login,
            access_token,
            expires_at,
        } => {
            let _lock = credential_lock("linear")?;
            if installed(store, Provider::Linear, &login).await? {
                return Ok(());
            }
            if linear_login(&access_token).await? != login {
                bail!("Linear credential identity mismatch");
            }
            store
                .upsert_provider_token(&ProviderToken {
                    provider: "linear".into(),
                    access_token,
                    refresh_token: None,
                    oauth_client_id: None,
                    expires_at,
                    login: Some(login),
                    updated_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    credential_type: CredentialType::OAuth,
                })
                .await?;
            Ok(())
        }
        Credential::OpenCode { login, entry } => {
            let _lock = credential_lock("opencode")?;
            if installed(store, Provider::OpenCodeZen, &login).await? {
                return Ok(());
            }
            if opencode_identity(&entry) != Some(login.as_str()) {
                bail!("OpenCode credential identity mismatch");
            }
            zen_check(opencode_token(&entry)).await?;
            let id = entry["id"]
                .as_str()
                .ok_or_else(|| anyhow!("OpenCode credential has no id"))?;
            // A same-id entry here is one Zen no longer accepts; the received one replaces it.
            let entries = opencode_export(None, "machine")?;
            if entries.iter().any(|existing| existing["id"] == id) {
                opencode_run(&["auth", "logout", "opencode", id])?;
            }
            let mut child = opencode(None, &["auth", "import"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            child
                .stdin
                .take()
                .expect("piped opencode stdin")
                .write_all(&serde_json::to_vec(&[&entry])?)?;
            if !child.wait()?.success() {
                bail!("OpenCode did not import the login");
            }
            opencode_run(&["auth", "switch", "opencode", id])?;
            if !installed(store, Provider::OpenCodeZen, &login).await? {
                bail!("OpenCode did not keep the imported login active");
            }
            Ok(())
        }
    }
}

fn opencode_run(args: &[&str]) -> Result<()> {
    let status = opencode(None, args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if !status.success() {
        bail!(
            "`opencode {}` failed (exit {:?})",
            args[..2].join(" "),
            status.code()
        );
    }
    Ok(())
}

/// Stored OpenCode Zen credentials on this `host`; an OpenCode without `auth export` is 1.x.
fn opencode_export(home: Option<&Path>, host: &str) -> Result<Vec<serde_json::Value>> {
    let missing = || {
        anyhow!("OpenCode 2 is not installed on this {host}; install it with `npm install -g @opencode/cli`")
    };
    // `auth export opencode` waits for input when that integration has no entry.
    let output = match opencode(home, &["auth", "export"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) if output.status.success() => output,
        Ok(_) => return Err(missing()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Err(missing()),
        Err(error) => return Err(error.into()),
    };
    let entries: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
        .map_err(|_| anyhow!("OpenCode returned an unreadable credential export"))?;
    Ok(entries
        .into_iter()
        .filter(|entry| entry["integrationID"] == loopflow_harness::HarnessKind::OpenCode.as_str())
        .collect())
}

fn opencode_active(entries: &[serde_json::Value]) -> Option<&serde_json::Value> {
    entries.iter().find(|entry| entry["active"] == true)
}

/// The OpenCode account behind a login; an API key names none, so it is its own identity.
fn opencode_identity(entry: &serde_json::Value) -> Option<&str> {
    entry["value"]["metadata"]["accountID"]
        .as_str()
        .or_else(|| entry["id"].as_str())
}

fn opencode_token(entry: &serde_json::Value) -> &str {
    entry["value"]["access"]
        .as_str()
        .or_else(|| entry["value"]["key"].as_str())
        .unwrap_or_default()
}

/// Zen accepting the bearer is the check; it names no owner.
async fn zen_check(token: &str) -> Result<()> {
    #[cfg(test)]
    let endpoint = std::env::var("LF_TEST_MACHINE_ZEN_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:1/zen/v1/models".into());
    #[cfg(not(test))]
    let endpoint = "https://opencode.ai/zen/v1/models";
    let response = reqwest::Client::new()
        .get(endpoint)
        .bearer_auth(token)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| anyhow!("OpenCode Zen request failed"))?;
    if response.status().is_success() {
        return Ok(());
    }
    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap_or_default();
    let reason = body["error"]["message"].as_str().unwrap_or(status.as_str());
    bail!("OpenCode Zen refused the login: {reason}")
}

/// One machine login per provider writes its account credentials at a time.
fn credential_lock(provider: &str) -> Result<fs::File> {
    let directory = crate::store::lf_home_dir().join("accounts");
    fs::create_dir_all(&directory)?;
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join(format!(".{provider}.machine-login.lock")))?;
    fs2::FileExt::try_lock_exclusive(&file).context("another machine login is in progress")?;
    Ok(file)
}

async fn github_login(token: &str) -> Result<String> {
    #[cfg(test)]
    let endpoint = std::env::var("LF_TEST_MACHINE_GITHUB_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:1/user".into());
    #[cfg(not(test))]
    let endpoint = "https://api.github.com/user";
    let response = reqwest::Client::new()
        .get(endpoint)
        .header("User-Agent", "loopflow")
        .bearer_auth(token)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|_| anyhow!("GitHub identity request failed"))?;
    if !response.status().is_success() {
        bail!("GitHub rejected credential identity request");
    }
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|_| anyhow!("invalid GitHub identity response"))?;
    value["login"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("GitHub identity missing login"))
}

async fn linear_login(token: &str) -> Result<String> {
    #[cfg(test)]
    let client = crate::linear::LinearClient::with_base_url(
        token.to_string(),
        None,
        std::env::var("LF_TEST_MACHINE_LINEAR_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:1/graphql".into()),
    );
    #[cfg(not(test))]
    let client = crate::linear::LinearClient::new(token.to_string(), None);
    client
        .viewer_id()
        .await
        .map_err(|_| anyhow!("Linear identity request failed"))
}
