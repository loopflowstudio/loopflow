//! Foreground login transfer. Only stdin carries credential bytes; saved machine
//! identity is checked again by the receiving process before it consumes them.
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{anyhow, bail, Context, Result};
use clap::Parser;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};

use crate::durable::Machine;
use crate::lf::{Cli, Commands, MachineCredentialCommand, SessionCommand, TaskCommand};
use crate::profile::EmailAddress;
use crate::provider_account::selection::AccountSelection;
use crate::provider_account::{
    account_home_path, account_id_for_login, account_login, acquire_managed_login_lock,
    ensure_account_home, match_account, new_account, open_account_store, AccountMatch,
};
use crate::provider_auth::Provider;
use crate::store::{CredentialType, ProviderAccount, ProviderToken, SharedStore};

// Deliberately no Debug: native credential JSON and tokens must never be logged.
#[derive(Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
enum Credential {
    Github {
        login: String,
        token: String,
    },
    Claude {
        login: String,
        subject: String,
        credential: String,
    },
    Codex {
        login: String,
        subject: String,
        credential: String,
    },
    Linear {
        login: String,
        access_token: String,
        refresh_token: String,
        oauth_client_id: String,
        expires_at: Option<i64>,
    },
}

pub(super) async fn connect(
    machine: &Machine,
    provider: Provider,
    email: Option<&str>,
    chrome_profile: Option<&str>,
) -> Result<()> {
    let store = open_account_store().await?;
    let credential = match provider {
        Provider::Claude | Provider::Codex => {
            let email = email.ok_or_else(|| anyhow!("{provider} requires a login email"))?;
            let account = managed_account(&store, provider, email).await?;
            let login = account_login(&account);
            if remote_has(machine, provider, login)? {
                return Ok(());
            }
            require_foreground()?;
            eprintln!(
                "Connecting {provider} {login} on {}",
                machine.label.as_deref().unwrap_or(&machine.route)
            );
            let staging =
                super::account::fresh_machine_login(&store, provider, &account, chrome_profile)
                    .await?;
            let (identity, _) =
                super::account::verify_managed_identity(&store, provider, &account, staging.path())
                    .await?;
            let credential = fs::read_to_string(staging.path().join(credential_file(provider)?))?;
            match provider {
                Provider::Claude => Credential::Claude {
                    login: login.into(),
                    subject: identity.subject,
                    credential,
                },
                _ => Credential::Codex {
                    login: login.into(),
                    subject: identity.subject,
                    credential,
                },
            }
        }
        Provider::GitHub => {
            // gh resolves its own selected token. It is never exported to a child.
            let output = Command::new("gh")
                .args(["auth", "token", "--hostname", "github.com"])
                .output()?;
            if !output.status.success() {
                bail!("GitHub is not connected on this laptop");
            }
            let token = SecretString::new(String::from_utf8(output.stdout)?.trim().to_string());
            let login = github_login(token.expose_secret()).await?;
            if email.is_some_and(|expected| !expected.eq_ignore_ascii_case(&login)) {
                bail!("GitHub login does not match the requested account");
            }
            if remote_has(machine, provider, &login)? {
                return Ok(());
            }
            require_foreground()?;
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
            if remote_has(machine, provider, &login)? {
                return Ok(());
            }
            require_foreground()?;
            let fresh = super::account::fresh_linear_login(&store, chrome_profile).await?;
            if linear_login(&fresh.access_token).await? != login {
                bail!("fresh Linear login belongs to a different account; discarded");
            }
            Credential::Linear {
                login,
                access_token: fresh.access_token,
                refresh_token: fresh
                    .refresh_token
                    .ok_or_else(|| anyhow!("Linear did not return a refresh token"))?,
                oauth_client_id: fresh
                    .oauth_client_id
                    .ok_or_else(|| anyhow!("Linear did not return a client identity"))?,
                expires_at: fresh.expires_at,
            }
        }
        _ => bail!("machine login supports github, claude, codex and linear"),
    };
    let bytes = serde_json::to_vec(&credential)?;
    remote_command(
        machine,
        &["machine", "credentials", "receive"],
        Some(&bytes),
    )?;
    eprintln!(
        "Connected {provider} on {}",
        machine.label.as_deref().unwrap_or(&machine.route)
    );
    Ok(())
}

fn require_foreground() -> Result<()> {
    use std::io::IsTerminal;
    // SAFETY: querying this process and stdin's foreground process group does not mutate either.
    let foreground = unsafe { libc::tcgetpgrp(libc::STDIN_FILENO) == libc::getpgrp() };
    if !std::io::stdin().is_terminal() || !foreground {
        bail!("a missing machine login needs a foreground terminal; run `lf machine connect <machine> <provider> [email]` there first");
    }
    Ok(())
}

async fn managed_account(
    store: &SharedStore,
    provider: Provider,
    selector: &str,
) -> Result<ProviderAccount> {
    let accounts = store
        .list_provider_accounts(Some(provider.as_str()))
        .await?;
    match match_account(&accounts.iter().collect::<Vec<_>>(), selector) {
        AccountMatch::One(account) => Ok(account.clone()),
        AccountMatch::Ambiguous(_) => bail!("ambiguous login; use its full email"),
        AccountMatch::None => {
            let email = EmailAddress::parse(selector).map_err(anyhow::Error::msg)?;
            let id = account_id_for_login(&email);
            Ok(new_account(
                provider,
                id.clone(),
                account_home_path(provider, &id)?,
                Some(email),
            ))
        }
    }
}

fn remote_has(machine: &Machine, provider: Provider, login: &str) -> Result<bool> {
    let output = remote_command(
        machine,
        &[
            "machine",
            "credentials",
            "inspect",
            provider.as_str(),
            login,
        ],
        None,
    )?;
    serde_json::from_slice(&output).context("remote credential inspection returned invalid JSON")
}

fn remote_command(machine: &Machine, args: &[&str], input: Option<&[u8]>) -> Result<Vec<u8>> {
    let script = format!(
        "{}; export {}={}; exec lf {}",
        super::machine::REMOTE_PATH,
        super::ssh::EXPECTED_MACHINE_ID_ENV,
        super::ssh::sh_quote(machine.id.as_str()),
        args.iter()
            .map(|arg| super::ssh::sh_quote(arg))
            .collect::<Vec<_>>()
            .join(" "),
    );
    let mut child = Command::new("ssh")
        .args(crate::engine::machine_route::bounded_ssh_args(
            &machine.route,
        ))
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
        // Never include a credential-consuming process's output in an error.
        bail!(
            "machine credential command failed on {} (exit {:?}); inspect its account status",
            machine.route,
            output.status.code()
        );
    }
    Ok(output.stdout)
}

pub(super) async fn receive(cmd: &MachineCredentialCommand) -> Result<()> {
    let store = open_account_store().await?;
    match cmd {
        MachineCredentialCommand::Inspect { provider, login } => {
            println!("{}", installed(&store, *provider, login).await?);
        }
        MachineCredentialCommand::Receive => {
            let mut bytes = Vec::new();
            std::io::stdin()
                .take(1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            if bytes.len() > 1024 * 1024 {
                bail!("credential exceeds transfer size limit");
            }
            let credential: Credential = serde_json::from_slice(&bytes)
                .map_err(|_| anyhow!("invalid credential transfer"))?;
            install(&store, credential).await?;
        }
    }
    Ok(())
}

async fn installed(store: &SharedStore, provider: Provider, login: &str) -> Result<bool> {
    match provider {
        Provider::Claude | Provider::Codex => {
            let account = managed_account(store, provider, login).await?;
            // Mere account metadata is not evidence of an installed credential.
            let Some(home) = account.home.as_ref() else {
                return Ok(false);
            };
            if !home.join(credential_file(provider)?).is_file() {
                return Ok(false);
            }
            // A transfer may have installed its file but lost the SQLite write.
            // Recover that same credential before asking for another browser approval.
            if store
                .get_provider_account(provider.as_str(), &account.account_id)
                .await?
                .is_none()
            {
                let _lock = acquire_managed_login_lock(home, provider, &account.account_id)?;
                let (identity, plan) =
                    super::account::verify_managed_identity(store, provider, &account, home)
                        .await?;
                super::account::register_managed_account(
                    store,
                    provider,
                    &account.account_id,
                    home.clone(),
                    identity,
                    plan,
                )
                .await?;
            }
            Ok(true)
        }
        Provider::GitHub => {
            let directory = std::env::var_os("GH_CONFIG_DIR")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .expect("OS user has a home")
                        .join(".config/gh")
                });
            let path = directory.join("hosts.yml");
            let content = match fs::read_to_string(path) {
                Ok(content) => content,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(error.into()),
            };
            let config: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content)
                .map_err(|_| anyhow!("invalid GitHub credential file"))?;
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
        _ => bail!("unsupported machine login provider"),
    }
}

async fn install(store: &SharedStore, credential: Credential) -> Result<()> {
    match credential {
        Credential::Claude {
            login,
            subject,
            credential,
        } => install_managed(store, Provider::Claude, &login, &subject, &credential).await,
        Credential::Codex {
            login,
            subject,
            credential,
        } => install_managed(store, Provider::Codex, &login, &subject, &credential).await,
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
                .spawn()?;
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
            refresh_token,
            oauth_client_id,
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
                    refresh_token: Some(refresh_token),
                    oauth_client_id: Some(oauth_client_id),
                    expires_at,
                    login: Some(login),
                    updated_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    credential_type: CredentialType::OAuth,
                })
                .await?;
            Ok(())
        }
    }
}

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

async fn install_managed(
    store: &SharedStore,
    provider: Provider,
    login: &str,
    subject: &str,
    credential: &str,
) -> Result<()> {
    let account = managed_account(store, provider, login).await?;
    let home = account_home_path(provider, &account.account_id)?;
    let parent = home.parent().expect("managed account home has a parent");
    fs::create_dir_all(parent)?;
    let _lock = acquire_managed_login_lock(&home, provider, &account.account_id)?;
    let file = credential_file(provider)?;
    if home.join(file).is_file() {
        // A prior transfer can have installed the file and lost its SQLite write.
        let (identity, plan) =
            super::account::verify_managed_identity(store, provider, &account, &home).await?;
        if identity.subject != subject {
            bail!("existing login has a different identity; left unchanged");
        }
        return super::account::register_managed_account(
            store,
            provider,
            &account.account_id,
            home,
            identity,
            plan,
        )
        .await;
    }
    let staging = tempfile::Builder::new()
        .prefix(".receive-")
        .tempdir_in(parent)?;
    write_private(&staging.path().join(file), credential.as_bytes())?;
    let (identity, plan) =
        super::account::verify_managed_identity(store, provider, &account, staging.path()).await?;
    if identity.subject != subject {
        bail!("received login has a different identity; discarded");
    }
    let _identity_lock = crate::provider_account::identity::acquire_identity_install_lock(&home)?;
    let home = ensure_account_home(provider, &account.account_id)?;
    // No replacement: a concurrently installed login remains its owner's.
    let bytes = fs::read(staging.path().join(file))?;
    write_private(&home.join(file), &bytes)?;
    super::account::register_managed_account(
        store,
        provider,
        &account.account_id,
        home,
        identity,
        plan,
    )
    .await
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("credential has no parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|_| anyhow!("credential already exists or cannot be installed"))?;
    Ok(())
}

fn credential_file(provider: Provider) -> Result<&'static str> {
    match provider {
        Provider::Claude => Ok(".credentials.json"),
        Provider::Codex => Ok("auth.json"),
        _ => bail!("unsupported managed provider"),
    }
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
    let client = crate::pm::linear::LinearClient::with_base_url(
        token.to_string(),
        None,
        std::env::var("LF_TEST_MACHINE_LINEAR_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:1/graphql".into()),
    );
    #[cfg(not(test))]
    let client = crate::pm::linear::LinearClient::new(token.to_string(), None);
    client
        .viewer_id()
        .await
        .map_err(|_| anyhow!("Linear identity request failed"))
}

pub(super) async fn prepare_launch(
    machine: &Machine,
    selection: &AccountSelection,
    args: &[String],
) -> Result<AccountSelection> {
    let argv = std::iter::once("lf".into())
        .chain(args.iter().cloned())
        .collect();
    let cli = Cli::try_parse_from(crate::lf::navigation::normalize_args(argv)?)?;
    let remote_selection = AccountSelection::from_flags(&cli.account, &cli.only_account)?;
    let origin_selection = selection;
    let selection = if remote_selection.is_default() {
        selection
    } else {
        &remote_selection
    };
    let store = open_account_store().await?;
    let catalog = store.list_provider_accounts(None).await?;
    let mut requested = selection.resolved_accounts(&catalog)?;
    if origin_selection.is_restricted() && !remote_selection.is_default() {
        let allowed = origin_selection.resolved_accounts(&catalog)?;
        if requested.iter().any(|account| !allowed.contains(account)) {
            bail!("remote account selection conflicts with the outer --only-account restriction");
        }
    }
    let launching = matches!(
        cli.command,
        None | Some(
            Commands::Skill { .. }
                | Commands::Run { .. }
                | Commands::External(_)
                | Commands::Task {
                    cmd: TaskCommand::Run { .. }
                }
                | Commands::Session {
                    cmd: SessionCommand::Resume { .. }
                }
        )
    );
    if requested.is_empty() && launching {
        let repo = super::util::find_repo_root().ok();
        let config = crate::engine::config::load_config_or_default(repo.as_deref());
        let model = cli.model.as_deref().unwrap_or_else(|| config.agent());
        let (harness, _) = crate::engine::config::parse_agent(model);
        let provider = harness.parse::<Provider>()?;
        let repo_id = crate::provider_account::current_repo_id()?;
        if let Some(accounts) = crate::provider_account::inspect_provider_route(
            Some(&store),
            repo_id.as_ref(),
            provider,
        )
        .await?
        {
            let account = accounts.first().ok_or_else(|| anyhow!("the laptop route for {provider} has no eligible login; select an account explicitly"))?;
            requested.push((provider, account.account_id.clone()));
        }
    }
    let mut selectors = Vec::new();
    for (provider, id) in requested {
        let account = catalog
            .iter()
            .find(|account| account.provider == provider.as_str() && account.account_id == id)
            .ok_or_else(|| anyhow!("selected account disappeared"))?;
        let login = account
            .login_email
            .as_ref()
            .ok_or_else(|| anyhow!("selected account has no expected login email"))?
            .as_str();
        connect(machine, provider, Some(login), None).await?;
        selectors.push(format!("{provider}={login}"));
    }
    // An explicitly requested login may not fall back to somebody else remotely.
    if selectors.is_empty() {
        Ok(selection.clone())
    } else {
        Ok(AccountSelection::from_flags(&[], &selectors)?)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::Arc;

    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use serde_json::json;

    use super::{install, install_managed, installed, write_private, Credential};
    use crate::provider_account::{account_home_path, account_id_for_login};
    use crate::provider_auth::Provider;
    use crate::store::{SharedStore, StorageConfig};

    struct Environment(Vec<(String, OsString)>);
    impl Environment {
        fn isolate(root: &Path) -> Self {
            let saved = Self(
                std::env::vars_os()
                    .map(|(key, value)| (key.to_string_lossy().into_owned(), value))
                    .collect(),
            );
            for (key, _) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("LF_")
                    || key.to_string_lossy().starts_with("LOOPFLOW_")
                {
                    std::env::remove_var(key);
                }
            }
            std::env::set_var("HOME", root);
            std::env::set_var("LF_HOME", root);
            std::env::set_var("GH_CONFIG_DIR", root.join("gh"));
            std::env::set_var("LF_PROVIDER_TOKEN_KEY_PATH", root.join("key"));
            std::env::set_var(
                "PATH",
                format!("{}:/usr/bin:/bin", root.join("bin").display()),
            );
            fs::create_dir_all(root.join("bin")).unwrap();
            saved
        }
    }
    impl Drop for Environment {
        fn drop(&mut self) {
            for (key, _) in std::env::vars_os() {
                std::env::remove_var(key);
            }
            for (key, value) in &self.0 {
                std::env::set_var(key, value);
            }
        }
    }
    async fn store(root: &Path) -> SharedStore {
        Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(root.join("loopflow.db")))
                .await
                .unwrap(),
        )
    }
    fn executable(path: &Path, text: &str) {
        fs::write(path, text).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn codex_credential(email: &str, token: &str) -> String {
        let claims = URL_SAFE_NO_PAD.encode(json!({"email": email, "sub": "user-123"}).to_string());
        json!({"tokens": {"access_token": token, "refresh_token": "fresh-refresh", "id_token": format!("header.{claims}.sig")}}).to_string()
    }
    fn codex(root: &Path) {
        executable(
            &root.join("bin/codex"),
            r#"#!/bin/sh
read -r initialize
echo '{"id":1,"result":{}}'
read -r initialized
read -r request
echo '{"id":2,"result":{"account":{"email":"person@example.com","planType":"plus"}}}'
"#,
        );
    }
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn received_codex_login_is_private_registered_and_never_replaced() {
        let _lock = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _environment = Environment::isolate(root.path());
        codex(root.path());
        let store = store(root.path()).await;
        let original = codex_credential("person@example.com", "fresh-for-target");
        install_managed(
            &store,
            Provider::Codex,
            "person@example.com",
            "user-123",
            &original,
        )
        .await
        .unwrap();
        let accounts = store.list_provider_accounts(Some("codex")).await.unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].observed_subject.as_deref(), Some("user-123"));
        let path = accounts[0].home.as_ref().unwrap().join("auth.json");
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        install_managed(
            &store,
            Provider::Codex,
            "person@example.com",
            "user-123",
            "discard-this-retry",
        )
        .await
        .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        assert_eq!(
            store
                .list_provider_accounts(Some("codex"))
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(installed(&store, Provider::Codex, "person@example.com")
            .await
            .unwrap());
        assert!(
            !installed(&store, Provider::Codex, "someone-else@example.com")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn interrupted_registration_reuses_the_received_native_login() {
        let _lock = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _environment = Environment::isolate(root.path());
        codex(root.path());
        let store = store(root.path()).await;
        let login = crate::profile::EmailAddress::parse("person@example.com").unwrap();
        let home = account_home_path(Provider::Codex, &account_id_for_login(&login)).unwrap();
        fs::create_dir_all(&home).unwrap();
        let credential = codex_credential("person@example.com", "already-received");
        write_private(&home.join("auth.json"), credential.as_bytes()).unwrap();
        assert!(installed(&store, Provider::Codex, login.as_str())
            .await
            .unwrap());
        assert_eq!(
            store
                .list_provider_accounts(Some("codex"))
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            fs::read_to_string(home.join("auth.json")).unwrap(),
            credential
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn selected_remote_launch_keeps_its_login_and_missing_login_stops_headless() {
        let _lock = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _environment = Environment::isolate(root.path());
        let store = store(root.path()).await;
        let account = super::managed_account(&store, Provider::Codex, "person@example.com")
            .await
            .unwrap();
        store.upsert_provider_account(&account).await.unwrap();
        let machine = crate::durable::Machine {
            id: crate::durable::MachineId::new(),
            route: "mini".into(),
            label: Some("mini".into()),
            repo: Some("~/src/project".into()),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            observed_at: time::OffsetDateTime::UNIX_EPOCH,
        };
        executable(&root.path().join("bin/ssh"), "#!/bin/sh\necho true\n");
        let selected = crate::provider_account::selection::AccountSelection::from_flags(
            &["codex=person@".into()],
            &[],
        )
        .unwrap();
        let prepared =
            super::prepare_launch(&machine, &selected, &["skill".into(), "implement".into()])
                .await
                .unwrap();
        assert!(prepared.is_restricted());
        assert_eq!(prepared.resolved_accounts(&[account]).unwrap().len(), 1);
        executable(&root.path().join("bin/ssh"), "#!/bin/sh\necho false\n");
        let error =
            super::prepare_launch(&machine, &selected, &["skill".into(), "implement".into()])
                .await
                .unwrap_err();
        assert!(error.to_string().contains("foreground terminal"));
        assert_eq!(store.list_provider_accounts(None).await.unwrap().len(), 1);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn wrong_login_is_discarded_before_account_registration() {
        let _lock = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _environment = Environment::isolate(root.path());
        codex(root.path());
        let store = store(root.path()).await;
        let error = install_managed(
            &store,
            Provider::Codex,
            "different@example.com",
            "user-123",
            &codex_credential("person@example.com", "discarded-token"),
        )
        .await
        .unwrap_err();
        assert!(!error.to_string().contains("discarded-token"));
        assert!(store.list_provider_accounts(None).await.unwrap().is_empty());
        let email = crate::profile::EmailAddress::parse("different@example.com").unwrap();
        assert!(
            !account_home_path(Provider::Codex, &account_id_for_login(&email))
                .unwrap()
                .join("auth.json")
                .exists()
        );
    }

    #[test]
    fn incomplete_transfer_and_retry_leave_existing_bytes_intact() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("auth.json");
        write_private(&path, b"retained").unwrap();
        assert!(write_private(&path, b"replacement").is_err());
        assert_eq!(fs::read(path).unwrap(), b"retained");
        assert!(serde_json::from_str::<Credential>(
            r#"{"provider":"codex","credential":"truncated"#
        )
        .is_err());
    }

    async fn identity_server(body: serde_json::Value) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    break;
                };
                let mut input = [0; 8192];
                let _ = stream.read(&mut input).await.unwrap();
                let body = body.to_string();
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        (endpoint, task)
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn github_installs_through_stdin_and_keeps_its_file_on_retry() {
        let _lock = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _environment = Environment::isolate(root.path());
        let (endpoint, server) = identity_server(json!({"login":"person"})).await;
        std::env::set_var("LF_TEST_MACHINE_GITHUB_URL", endpoint);
        executable(
            &root.path().join("bin/gh"),
            r#"#!/bin/sh
case "$*" in
  *fixture-github-token*) exit 91;;
esac
[ -z "$GH_TOKEN$GITHUB_TOKEN" ] || exit 92
if [ "$2" = setup-git ]; then exit 0; fi
[ "$*" = 'auth login --hostname github.com --git-protocol https --insecure-storage --with-token' ] || exit 93
token=$(cat)
mkdir -p "$GH_CONFIG_DIR"
printf 'github.com:\n  user: person\n  oauth_token: %s\n' "$token" > "$GH_CONFIG_DIR/hosts.yml"
chmod 600 "$GH_CONFIG_DIR/hosts.yml"
"#,
        );
        let store = store(root.path()).await;
        install(
            &store,
            Credential::Github {
                login: "person".into(),
                token: "fixture-github-token".into(),
            },
        )
        .await
        .unwrap();
        install(
            &store,
            Credential::Github {
                login: "person".into(),
                token: "ignored-retry".into(),
            },
        )
        .await
        .unwrap();
        assert!(fs::read_to_string(root.path().join("gh/hosts.yml"))
            .unwrap()
            .contains("fixture-github-token"));
        assert!(installed(&store, Provider::GitHub, "person").await.unwrap());
        assert!(!installed(&store, Provider::GitHub, "another")
            .await
            .unwrap());
        server.abort();
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn claude_installs_its_native_file_without_keychain_access() {
        let _lock = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _environment = Environment::isolate(root.path());
        let (endpoint, server) = identity_server(
            json!({"account":{"email":"person@example.com","uuid":"claude-person"}}),
        )
        .await;
        std::env::set_var("LF_TEST_CLAUDE_PROFILE_URL", endpoint);
        executable(&root.path().join("bin/security"), "#!/bin/sh\nexit 99\n");
        let store = store(root.path()).await;
        let credential = json!({"claudeAiOauth": {"accessToken":"target-access","refreshToken":"target-refresh","expiresAt":4102444800000i64}}).to_string();
        install_managed(
            &store,
            Provider::Claude,
            "person@example.com",
            "claude-person",
            &credential,
        )
        .await
        .unwrap();
        let accounts = store.list_provider_accounts(Some("claude")).await.unwrap();
        assert_eq!(accounts.len(), 1);
        let path = accounts[0].home.as_ref().unwrap().join(".credentials.json");
        assert_eq!(fs::read_to_string(&path).unwrap(), credential);
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        server.abort();
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn linear_retains_an_independent_encrypted_refresh_grant() {
        let _lock = crate::journal::test_env_lock();
        let root = tempfile::tempdir().unwrap();
        let _environment = Environment::isolate(root.path());
        let (endpoint, server) =
            identity_server(json!({"data":{"viewer":{"id":"viewer-123"}}})).await;
        std::env::set_var("LF_TEST_MACHINE_LINEAR_URL", endpoint);
        let store = store(root.path()).await;
        let grant = |refresh: &str| Credential::Linear {
            login: "viewer-123".into(),
            access_token: "fresh-access".into(),
            refresh_token: refresh.into(),
            oauth_client_id: "fixture-client".into(),
            expires_at: Some(4102444800),
        };
        install(&store, grant("target-refresh-chain"))
            .await
            .unwrap();
        install(&store, grant("ignored-retry")).await.unwrap();
        let token = store.get_provider_token("linear").await.unwrap().unwrap();
        assert_eq!(token.refresh_token.as_deref(), Some("target-refresh-chain"));
        assert_eq!(token.oauth_client_id.as_deref(), Some("fixture-client"));
        let db = rusqlite::Connection::open(root.path().join("loopflow.db")).unwrap();
        let (refresh, encrypted): (String, bool) = db
            .query_row(
                "SELECT refresh_token, encrypted FROM provider_tokens WHERE provider='linear'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(encrypted);
        assert!(!refresh.contains("target-refresh-chain"));
        assert!(installed(&store, Provider::Linear, "other-person")
            .await
            .is_err());
        server.abort();
    }
}
