//! `lf machine login`: each provider's own sign-in, run on the Machine that
//! keeps the login and approved where `approvers.<provider>` says.
//!
//! For Codex the target runs `codex login --device-auth` unmodified, locally
//! as a child or behind one SSH exchange without a PTY; lf reads its approval
//! page and code and sends them to the approver. Claude signs in here only,
//! with `claude auth login` in this terminal: another Machine runs Claude on
//! the `claude setup-token` that `forward.env` carries as
//! `CLAUDE_CODE_OAUTH_TOKEN`. lf stores nothing: each client writes and
//! refreshes its own login.
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::placement::Approver;
use crate::provider_auth::Provider;
use loopflow_model::durable::Machine;

/// How long a sign-in may wait for its approval: Codex's device code lifetime.
const APPROVAL_DEADLINE: Duration = Duration::from_secs(15 * 60);
/// How long the login may take to print its approval page, SSH included.
const PAGE_DEADLINE: Duration = Duration::from_secs(60);
/// The `claude setup-token` a launch on another Machine signs Claude in with.
pub const CLAUDE_TOKEN_ENV: &str = "CLAUDE_CODE_OAUTH_TOKEN";

/// Sign `machine` in to `provider`. A Machine whose route is `local` is this
/// one. A rental's first GitHub sign-in is for the repository `repo` is a
/// checkout of; nothing else reads `repo`.
pub async fn login(
    machine: &Machine,
    provider: Provider,
    repo: &std::path::Path,
    chrome_profile: Option<&str>,
    api_key: bool,
    batch: bool,
) -> Result<()> {
    match provider {
        Provider::Claude if here(machine) => claude_here(machine),
        Provider::Codex => codex(machine, approver(provider, chrome_profile)?.as_ref()),
        service if here(machine) => {
            crate::provider_auth::operations::connect(service, chrome_profile, api_key).await
        }
        service => {
            require_reach(service, Some(&name(machine)))?;
            let approver = approver(service, chrome_profile)?;
            crate::machine::credentials::connect(machine, service, repo, approver.as_ref(), batch)
                .await
        }
    }
}

/// Whether `lf machine login` can sign another Machine in to `provider`:
/// Codex runs its own login there, and GitHub, Linear and OpenCode send a
/// credential. Claude arrives with each launch as a forwarded token; Asana
/// and Doppler sign in on each Machine itself.
pub fn reaches_other_machines(provider: Provider) -> bool {
    matches!(
        provider,
        Provider::Codex | Provider::GitHub | Provider::Linear | Provider::OpenCodeZen
    )
}

/// Whether a rental is signed in to `provider` because its repository uses
/// it, so no template lists it.
pub fn follows_repository(provider: Provider) -> bool {
    matches!(provider, Provider::GitHub | Provider::Linear)
}

/// Refuses a provider that cannot sign `machine` (another Machine, or any
/// when `None`) in, before anything connects to one.
pub fn require_reach(provider: Provider, machine: Option<&str>) -> Result<()> {
    if provider == Provider::Claude {
        bail!(
            "Claude reaches {} as a forwarded token: run `claude setup-token`, store it with \
             `doppler secrets set {CLAUDE_TOKEN_ENV}`, and add {CLAUDE_TOKEN_ENV} to `forward.env`",
            machine.unwrap_or("other Machines")
        );
    }
    if !reaches_other_machines(provider) {
        bail!(
            "{} signs in on each Machine itself; run `lf machine login {}` there",
            provider.display_name(),
            provider.as_str(),
        );
    }
    Ok(())
}

/// Claude's own login in this terminal, with its own page.
fn claude_here(machine: &Machine) -> Result<()> {
    let status = Command::new("claude")
        .args(["auth", "login"])
        .status()
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => not_installed(Provider::Claude, &name(machine)),
            _ => anyhow::Error::from(error).context("start Claude's login"),
        })?;
    let name = name(machine);
    match status.code() {
        Some(0) => {}
        // Claude's own answer to Ctrl-C.
        Some(130) => bail!("Claude's login was interrupted; {name} was not signed in"),
        _ => bail!("Claude's login exited with {status}; {name} was not signed in"),
    }
    match confirm(machine, Provider::Claude, LoginState::SignedIn, "login")? {
        Some(account) => println!("{name}: Claude signed in as {account}"),
        None => println!("{name}: Claude signed in"),
    }
    Ok(())
}

/// Device code login: the person, or the approver command, enters the code
/// Codex prints; Codex polls and writes its own login when it is approved.
fn codex(machine: &Machine, approver: Option<&Approver>) -> Result<()> {
    let name = name(machine);
    let mut login = Login::start(machine)?;
    let url = login.next(url_in)?;
    let code = login.next(device_code_in)?;
    let page = Page {
        provider: Provider::Codex,
        machine: &name,
        url: &url,
        code: Some(&code),
    };
    match send(approver, &page) {
        Ok(Sent::Opened(place)) => {
            eprintln!("Approve Codex for {name} {place} with code {code}")
        }
        Ok(Sent::Approved) => {}
        Err(error) => {
            login.kill();
            return Err(error);
        }
    }
    login.finish(&code)?;
    let account = confirm(machine, Provider::Codex, LoginState::SignedIn, "login")?;
    println!(
        "{name}: Codex signed in ({})",
        account.as_deref().unwrap_or("no account named")
    );
    Ok(())
}

/// The client's own answer once its login or logout has run: `want`, with the
/// account it names, or the error.
fn confirm(
    machine: &Machine,
    provider: Provider,
    want: LoginState,
    ran: &str,
) -> Result<Option<String>> {
    let row = probe(machine, provider, PROBE_DEADLINE);
    if row.state == want {
        return Ok(row.account);
    }
    let (display, name) = (provider.display_name(), name(machine));
    match row.state {
        LoginState::Unknown => bail!("{display}'s {ran} ran but {name} could not say who it is"),
        state => bail!(
            "{display}'s {ran} ran but {name} reports {}",
            state.as_str()
        ),
    }
}

/// Sign `machine` out of `provider` with the provider's own logout; its client
/// saying it is signed out afterwards is the result.
pub async fn logout(machine: &Machine, provider: Provider) -> Result<()> {
    let name = name(machine);
    let display = provider.display_name();
    let Some(argv) = logout_argv(provider) else {
        if here(machine) {
            crate::provider_auth::operations::disconnect(provider).await?;
        } else {
            crate::machine::credentials::remote_command(
                machine,
                &["machine", "logout", provider.as_str()],
                None,
            )?;
        }
        println!("{name}: {display} signed out");
        return Ok(());
    };
    match crate::os_process::bounded_output(&mut command(machine, argv)?, PROBE_DEADLINE) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(not_installed(provider, &name))
        }
        Ok(output) if output.status.code() == Some(127) => {
            return Err(not_installed(provider, &name))
        }
        _ => {}
    }
    confirm(machine, provider, LoginState::SignedOut, "logout")?;
    println!("{name}: {display} signed out");
    Ok(())
}

fn logout_argv(provider: Provider) -> Option<&'static [&'static str]> {
    match provider {
        Provider::Claude => Some(&["claude", "auth", "logout"]),
        Provider::Codex => Some(&["codex", "logout"]),
        _ => None,
    }
}

/// The added Machines `lf machine login <provider> --refused` signs in: those
/// whose `provider` login reads signed out, or refused a launch with none
/// since. A Machine that cannot answer comes back as its error.
pub async fn refused(provider: Provider) -> Result<Vec<(String, Result<Machine>)>> {
    let store = crate::provider_auth::operations::local_store().await?;
    let mut selected = Vec::new();
    for machine in store.machines().await? {
        let name = name(&machine);
        match logins(&machine).await {
            Ok(rows) => {
                if rows.iter().any(|row| {
                    row.provider == provider
                        && (row.state == LoginState::SignedOut || row.refused.is_some())
                }) {
                    selected.push((name, Ok(machine)));
                }
            }
            Err(error) => selected.push((name, Err(error))),
        }
    }
    Ok(selected)
}

fn url_in(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    line[start..].split_whitespace().next().map(str::to_string)
}

/// Codex's one-time code, alone on its line: `ABCD-1234`.
fn device_code_in(line: &str) -> Option<String> {
    let code = line.trim();
    let (left, right) = code.split_once('-')?;
    let part = |part: &str| {
        (4..=8).contains(&part.len())
            && part
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
    };
    (part(left) && part(right)).then(|| code.to_string())
}

// ---------------------------------------------------------------------------
// What a Machine is signed in to

/// One provider's login on a Machine, as its own client or store reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoginRow {
    pub provider: Provider,
    pub state: LoginState,
    /// The account the client names: Claude's email and plan, a service
    /// login, or the variable a forwarded login arrives in.
    #[serde(deserialize_with = "Option::deserialize")]
    pub account: Option<String>,
    /// The provider's last refusal here, when no launch of it has come since.
    #[serde(deserialize_with = "Option::deserialize")]
    pub refused: Option<Refusal>,
}

/// A launch the provider refused: no login, a usage limit, a model it would not run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    pub reason: String,
    /// Unix seconds.
    pub at: i64,
}

/// `unknown`: the client could not answer. Shown, never refused on.
/// `forwarded`: each launch there carries the login from here (Claude's
/// `CLAUDE_CODE_OAUTH_TOKEN`), which the Machine's own client cannot see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginState {
    SignedIn,
    SignedOut,
    Forwarded,
    Unknown,
}

impl LoginState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignedIn => "signed in",
            Self::SignedOut => "signed out",
            Self::Forwarded => "forwarded",
            Self::Unknown => "unknown",
        }
    }
}

/// Every provider's login on `machine`. Another Machine answers with its own
/// `lf machine logins --json`, so its clients and store are the ones asked;
/// its Claude row reads `forwarded` when this Machine's `forward.env` names
/// `CLAUDE_CODE_OAUTH_TOKEN` and this Machine has its value, and says so when
/// the name has no value here.
pub async fn logins(machine: &Machine) -> Result<Vec<LoginRow>> {
    if !here(machine) {
        let output = crate::machine::credentials::remote_command(
            machine,
            &["machine", "logins", "--json"],
            None,
        )?;
        let mut rows: Vec<LoginRow> = serde_json::from_slice(&output)
            .with_context(|| format!("{}'s logins were unreadable", name(machine)))?;
        // Only a token this Machine can supply reaches a launch from here.
        if let Ok(forwarded) = crate::machine::forward::read(&std::env::current_dir()?) {
            for row in rows
                .iter_mut()
                .filter(|row| row.provider == Provider::Claude)
            {
                if forwarded.carries_env(CLAUDE_TOKEN_ENV) {
                    row.state = LoginState::Forwarded;
                    row.account = Some(CLAUDE_TOKEN_ENV.to_string());
                } else if forwarded.lacks_claude_token() {
                    row.account = Some(format!("{CLAUDE_TOKEN_ENV} has no value here"));
                }
            }
        }
        return Ok(rows);
    }
    let store = crate::provider_auth::operations::local_store().await?;
    let mut rows = Vec::new();
    // The agents first, then the services.
    let agents = [Provider::Claude, Provider::Codex];
    let services = Provider::all()
        .into_iter()
        .filter(|provider| !agents.contains(provider));
    for provider in agents.into_iter().chain(services) {
        let mut row = match provider {
            Provider::Claude | Provider::Codex => probe(machine, provider, PROBE_DEADLINE),
            service => service_login(&store, service).await?,
        };
        row.refused = crate::store::run_sqlite(&store.sqlite, move |store| {
            store.unanswered_refusal(provider.as_str())
        })
        .await?
        .map(|(reason, at)| Refusal { reason, at });
        rows.push(row);
    }
    Ok(rows)
}

/// A service's login where its client keeps one (gh's hosts file, OpenCode's
/// own store), else what this Machine's store saved for it.
async fn service_login(store: &crate::store::SharedStore, provider: Provider) -> Result<LoginRow> {
    let installed = match provider {
        Provider::GitHub => crate::machine::credentials::github_user()?,
        Provider::OpenCodeZen => crate::machine::credentials::opencode_login(),
        _ => None,
    };
    let (state, account) = match installed {
        // A rental's GitHub token expires: signed in means GitHub takes it.
        Some(account) if provider == Provider::GitHub => {
            let state = match crate::machine::credentials::github_token_works(&account).await? {
                Some(true) => LoginState::SignedIn,
                Some(false) => LoginState::SignedOut,
                None => LoginState::Unknown,
            };
            (state, Some(account))
        }
        Some(account) => (LoginState::SignedIn, Some(account)),
        None => match store.provider_auth_snapshot(provider).await? {
            Some(snapshot) if snapshot.status.as_str() == "active" => {
                (LoginState::SignedIn, snapshot.status.login())
            }
            _ => (LoginState::SignedOut, None),
        },
    };
    Ok(LoginRow {
        provider,
        state,
        account,
        refused: None,
    })
}

pub async fn print_logins(machine: &Machine, json: bool) -> Result<()> {
    let rows = logins(machine).await?;
    if json {
        println!("{}", serde_json::to_string(&rows)?);
        return Ok(());
    }
    for row in rows {
        let mut line = format!(
            "{:<12} {:<11}  {}",
            row.provider.as_str(),
            row.state.as_str(),
            row.account.unwrap_or_default()
        );
        if let Some(refusal) = row.refused {
            line.push_str(&format!(
                "  refused {}",
                crate::output::format_time(refusal.at)
            ));
        }
        println!("{}", line.trim_end());
    }
    Ok(())
}

/// Asks `provider`'s own client on `machine` who is signed in. A client that
/// is missing, slower than `deadline` or unreadable answers `unknown`.
pub fn probe(machine: &Machine, provider: Provider, deadline: Duration) -> LoginRow {
    let (state, account) = status_argv(provider)
        .and_then(|argv| command(machine, argv).ok())
        .map_or((LoginState::Unknown, None), |mut command| {
            answer(provider, &mut command, deadline)
        });
    LoginRow {
        provider,
        state,
        account,
        refused: None,
    }
}

/// The launch check: `mini has no Codex login` when `harness`'s client here,
/// with the launch's `env`, definitely has no login. Anything short of a
/// definite answer launches: lf never refuses a working login on its own
/// misreading.
pub fn missing_login(
    harness: &str,
    env: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    let provider = harness.parse::<Provider>().ok()?;
    let mut command = local_command(status_argv(provider)?);
    command.envs(env);
    for key in crate::provider_auth::api_key_env_names() {
        command.env_remove(key);
    }
    let started = std::time::Instant::now();
    let (state, _) = answer(provider, &mut command, LAUNCH_PROBE_DEADLINE);
    tracing::debug!(
        harness,
        state = state.as_str(),
        elapsed_ms = started.elapsed().as_millis(),
        "login probe"
    );
    (state == LoginState::SignedOut).then(|| {
        let machine = launcher_name().unwrap_or_else(|| "This Machine".to_string());
        format!("{machine} has no {} login", provider.display_name())
    })
}

/// The name the launching Machine gives this one, so the sign-in advice a
/// launch here prints is the command that works where it is read.
pub const MACHINE_NAME_ENV: &str = "LF_MACHINE_NAME";

fn launcher_name() -> Option<String> {
    std::env::var(MACHINE_NAME_ENV)
        .ok()
        .filter(|name| !name.is_empty())
}

/// What signs this Machine in to `provider`, as whoever launched here would
/// do it: Claude on another Machine is a forwarded token, the rest a sign-in.
pub fn sign_in_advice(provider: Provider) -> String {
    match (launcher_name(), provider) {
        (Some(_), Provider::Claude) => format!(
            "launch where {CLAUDE_TOKEN_ENV} has a value (`doppler secrets set {CLAUDE_TOKEN_ENV}`, \
             or run under `doppler run`) and `forward.env` names it \
             (`lf set --global forward.env`)"
        ),
        (Some(name), provider) => format!("run `lf machine login {} {name}`", provider.as_str()),
        (None, provider) => format!("run `lf machine login {}`", provider.as_str()),
    }
}

/// How long `logins` and a finished sign-in wait for a client, SSH included.
const PROBE_DEADLINE: Duration = Duration::from_secs(30);
/// How long a launch waits for its login probe before launching anyway.
/// `claude auth status` took 0.34–0.39 s here (2026-11-17), Codex 0.05 s.
const LAUNCH_PROBE_DEADLINE: Duration = Duration::from_secs(2);

fn status_argv(provider: Provider) -> Option<&'static [&'static str]> {
    match provider {
        Provider::Claude => Some(&["claude", "auth", "status", "--json"]),
        Provider::Codex => Some(&["codex", "login", "status"]),
        _ => None,
    }
}

/// `claude auth status --json` exits 1 with `loggedIn: false` when signed out;
/// `codex login status` exits 0 `Logged in using ChatGPT`, 1 `Not logged in`.
fn answer(
    provider: Provider,
    command: &mut Command,
    deadline: Duration,
) -> (LoginState, Option<String>) {
    let Ok(output) = crate::os_process::bounded_output(command, deadline) else {
        return (LoginState::Unknown, None);
    };
    match provider {
        Provider::Claude => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Claude {
                logged_in: bool,
                email: Option<String>,
                subscription_type: Option<String>,
            }
            match serde_json::from_slice::<Claude>(&output.stdout) {
                Ok(Claude {
                    logged_in: true,
                    email,
                    subscription_type,
                }) => (
                    LoginState::SignedIn,
                    match (email, subscription_type) {
                        (Some(email), Some(plan)) => Some(format!("{email} ({plan})")),
                        (email, plan) => email.or(plan),
                    },
                ),
                Ok(_) => (LoginState::SignedOut, None),
                Err(_) => (LoginState::Unknown, None),
            }
        }
        Provider::Codex => {
            let said = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            match output.status.code() {
                // An API key login reads `Logged in using an API key - sk-…`; only the method crosses.
                Some(0) => (
                    LoginState::SignedIn,
                    said.lines().find_map(|line| {
                        let method = line.trim().strip_prefix("Logged in using ")?;
                        Some(method.split(" - ").next().unwrap_or(method).to_string())
                    }),
                ),
                Some(1) if said.contains("Not logged in") => (LoginState::SignedOut, None),
                _ => (LoginState::Unknown, None),
            }
        }
        _ => (LoginState::Unknown, None),
    }
}

// ---------------------------------------------------------------------------
// The approver

/// `--chrome-profile` for this run, else `approvers.<provider>`; none opens
/// the default browser.
pub fn approver(provider: Provider, chrome_profile: Option<&str>) -> Result<Option<Approver>> {
    if let Some(profile) = chrome_profile {
        return Ok(Some(Approver::Chrome {
            chrome: profile.to_string(),
        }));
    }
    let approvers = crate::config::load_config(None)?
        .unwrap_or_default()
        .approvers;
    Ok(approvers.into_iter().find_map(|(name, approver)| {
        (name.parse::<Provider>().ok() == Some(provider)).then_some(approver)
    }))
}

/// One approval page a sign-in shows.
pub(crate) struct Page<'a> {
    pub provider: Provider,
    pub machine: &'a str,
    pub url: &'a str,
    /// The device code the page asks for, when it asks for one.
    pub code: Option<&'a str>,
}

pub(crate) enum Sent {
    /// Opened for a person to approve; says where.
    Opened(String),
    /// The approver command approved it.
    Approved,
}

/// Sends `page` to `approver`. A page that could not open is printed instead.
pub(crate) fn send(approver: Option<&Approver>, page: &Page) -> Result<Sent> {
    let (opened, place) = match approver {
        None => (
            crate::platform::open_url_checked(page.url).map_err(anyhow::Error::from),
            "in your browser".to_string(),
        ),
        Some(Approver::Chrome { chrome: requested }) => {
            let profile = crate::profile::resolve_local_chrome_profile(requested)
                .map_err(anyhow::Error::msg)?;
            (
                crate::provider_auth::operations::open_chrome_profile(&profile, page.url),
                format!("in Chrome ({})", profile.name),
            )
        }
        Some(Approver::Command { command }) => {
            return run_approver(command, page).map(|()| Sent::Approved)
        }
    };
    if let Err(error) = opened {
        eprintln!(
            "Could not open the approval page ({error}); open it yourself:\n  {}",
            page.url
        );
        return Ok(Sent::Opened("there".to_string()));
    }
    Ok(Sent::Opened(place))
}

/// `sh -c <command>` with the page in `LF_APPROVE_*`; its stderr is the person's.
fn run_approver(command: &str, page: &Page) -> Result<()> {
    let mut approve = Command::new("sh");
    approve
        .arg("-c")
        .arg(command)
        .env("LF_APPROVE_PROVIDER", page.provider.as_str())
        .env("LF_APPROVE_URL", page.url)
        .env("LF_APPROVE_MACHINE", page.machine)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    match page.code {
        Some(code) => approve.env("LF_APPROVE_CODE", code),
        None => approve.env_remove("LF_APPROVE_CODE"),
    };
    let status = approve
        .status()
        .with_context(|| format!("run the approver for {}", page.provider.display_name()))?;
    if !status.success() {
        bail!(
            "the approver for {} exited with {status}; {} was not signed in",
            page.provider.display_name(),
            page.machine
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Codex's device login process on the target

struct Login {
    child: Child,
    target: String,
    /// Each output line, and whether it came from stderr.
    lines: mpsc::Receiver<(bool, String)>,
    /// The login's recent stderr, for its error line; never shown as a whole.
    errors: Vec<String>,
    started: SystemTime,
}

impl Login {
    fn start(machine: &Machine) -> Result<Self> {
        let target = name(machine);
        let tied = [
            "sh",
            "-c",
            TIED_TO_STDIN,
            "sh",
            "codex",
            "login",
            "--device-auth",
        ];
        let mut child = command(machine, &tied)?
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => not_installed(Provider::Codex, &target),
                _ => anyhow::Error::from(error).context("start Codex's login"),
            })?;
        let (sender, lines) = mpsc::channel();
        for (stderr, stream) in [
            (
                false,
                Box::new(child.stdout.take().expect("piped stdout")) as Box<dyn Read + Send>,
            ),
            (true, Box::new(child.stderr.take().expect("piped stderr"))),
        ] {
            let sender = sender.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stream).lines().map_while(Result::ok) {
                    if sender
                        .send((stderr, crate::provider_auth::strip_ansi_escape_codes(&line)))
                        .is_err()
                    {
                        break;
                    }
                }
            });
        }
        Ok(Self {
            child,
            target,
            lines,
            errors: Vec::new(),
            started: SystemTime::now(),
        })
    }

    /// The first output line `find` accepts, as `find` returns it.
    fn next(&mut self, find: fn(&str) -> Option<String>) -> Result<String> {
        let deadline = SystemTime::now() + PAGE_DEADLINE;
        loop {
            let left = deadline
                .duration_since(SystemTime::now())
                .unwrap_or_default();
            match self.lines.recv_timeout(left) {
                Ok((stderr, line)) => {
                    if let Some(found) = find(&line) {
                        return Ok(found);
                    }
                    self.remember((stderr, line));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    self.kill();
                    bail!("Codex's login on {} printed no approval page", self.target);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(self.exited()),
            }
        }
    }

    /// Waits for the login to exit successfully, killing it at the deadline.
    /// Its stdin stays open meanwhile, since closing it stops the login.
    fn finish(&mut self, code: &str) -> Result<()> {
        loop {
            while let Ok(line) = self.lines.try_recv() {
                self.remember(line);
            }
            if let Some(status) = self.child.try_wait()? {
                if status.success() {
                    return Ok(());
                }
                // Lines still in flight carry the error.
                while let Ok(line) = self.lines.recv_timeout(Duration::from_millis(200)) {
                    self.remember(line);
                }
                return Err(self.failure(status.code(), code));
            }
            if self.started.elapsed().unwrap_or_default() > APPROVAL_DEADLINE {
                self.kill();
                bail!(
                    "Codex was not approved within 15 minutes; {} was not signed in",
                    self.target
                );
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn exited(&mut self) -> anyhow::Error {
        match self.child.wait() {
            Ok(status) => self.failure(status.code(), ""),
            Err(error) => error.into(),
        }
    }

    fn failure(&self, exit: Option<i32>, code: &str) -> anyhow::Error {
        // `sh: codex: command not found` over SSH.
        if exit == Some(127) {
            return not_installed(Provider::Codex, &self.target);
        }
        // The last line that quotes neither the page nor the code.
        let reason = self
            .errors
            .iter()
            .rev()
            .map(|line| line.trim())
            .find(|line| {
                !line.is_empty()
                    && !line.contains("http")
                    && (code.is_empty() || !line.contains(code))
            })
            .map(|line| format!(": {line}"))
            .unwrap_or_default();
        anyhow!(
            "Codex's login on {} failed{reason}; device code login must be enabled in \
             ChatGPT security settings",
            self.target
        )
    }

    fn remember(&mut self, (stderr, line): (bool, String)) {
        if !stderr {
            return;
        }
        if self.errors.len() == 8 {
            self.errors.remove(0);
        }
        self.errors.push(line);
    }

    fn kill(&mut self) {
        // Closing stdin stops the login wherever it runs (`TIED_TO_STDIN`).
        drop(self.child.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Login {
    /// Ctrl-C or an early return never leaves a login waiting on the target.
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            self.kill();
        }
    }
}

fn not_installed(provider: Provider, target: &str) -> anyhow::Error {
    let client = match provider {
        Provider::Claude => "Claude Code",
        Provider::Codex => "Codex",
        other => other.display_name(),
    };
    anyhow!(
        "{client} is not installed on {target}; install it there, then run `lf machine login {} {target}` again",
        provider.as_str()
    )
}

// ---------------------------------------------------------------------------
// Running a client on the target

/// Runs a login (`"$@"`) that stops when its stdin closes: when lf finishes,
/// is interrupted or loses the connection. Closing an SSH exchange without a
/// PTY signals nothing remote, and `codex login --device-auth` would keep
/// polling. The login reads stdin through a FIFO; the reader
/// copying stdin into it kills the login at EOF.
const TIED_TO_STDIN: &str = r#"d=$(mktemp -d) && mkfifo "$d/in" || exit 1
"$@" <"$d/in" & login=$!
exec 3>"$d/in" 4<&0
rm -rf "$d"
{ cat <&4 >&3; kill "$login"; } >/dev/null 2>&1 &
exec 3>&- 4<&-
wait "$login""#;

/// `argv` on `machine`: a child here, one SSH exchange without a PTY there.
/// `BROWSER=true` keeps the client from opening a window of its own.
fn command(machine: &Machine, argv: &[&str]) -> Result<Command> {
    if here(machine) {
        return Ok(local_command(argv));
    }
    let script = format!(
        "{}; BROWSER=true exec {}",
        crate::machine::ssh::remote_path(),
        argv.iter()
            .map(|arg| crate::machine::ssh::sh_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let mut command = Command::new("ssh");
    command
        .args(crate::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(script);
    Ok(command)
}

fn local_command(argv: &[&str]) -> Command {
    let mut command = Command::new(argv[0]);
    command.args(&argv[1..]).env("BROWSER", "true");
    command
}

fn here(machine: &Machine) -> bool {
    machine.route == "local"
}

pub(crate) fn name(machine: &Machine) -> String {
    match &machine.label {
        Some(label) => label.clone(),
        None if here(machine) => crate::store::ownership::local_label(),
        None => machine.route.clone(),
    }
}
