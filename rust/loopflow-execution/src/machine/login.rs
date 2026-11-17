//! `lf machine login`: each provider's own sign-in, run on the Machine that
//! keeps the login and approved where `approvers.<provider>` says.
//!
//! For Claude and Codex the target runs the client's login unmodified
//! (`claude auth login`, `codex login --device-auth`), locally as a child or
//! behind one SSH exchange without a PTY. lf reads its approval page, sends it
//! to the approver and, for Claude, writes the code the page shows to the
//! login's stdin. That code is useless without the PKCE verifier inside the
//! `claude` process, and lf stores nothing: each client writes and refreshes
//! its own login.
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::config::placement::Approver;
use crate::provider_auth::Provider;
use loopflow_model::durable::Machine;

/// How long a sign-in may wait for its approval: Codex's device code lifetime,
/// which Claude shares so both fail the same way.
const APPROVAL_DEADLINE: Duration = Duration::from_secs(15 * 60);
/// How long the login may take to print its approval page, SSH included.
const PAGE_DEADLINE: Duration = Duration::from_secs(60);

/// Sign `machine` in to `provider`. A Machine whose route is `local` is this one.
pub async fn login(
    machine: &Machine,
    provider: Provider,
    chrome_profile: Option<&str>,
    api_key: bool,
    batch: bool,
) -> Result<()> {
    if here(machine) && !matches!(provider, Provider::Claude | Provider::Codex) {
        return crate::provider_auth::operations::connect_target(
            provider.as_str(),
            chrome_profile,
            api_key,
        )
        .await;
    }
    let approver = approver(provider, chrome_profile)?;
    match provider {
        Provider::Claude => claude(machine, approver.as_ref(), batch),
        Provider::Codex => codex(machine, approver.as_ref()),
        _ => {
            crate::machine::credentials::connect(machine, provider, approver.as_ref(), batch).await
        }
    }
}

fn claude(machine: &Machine, approver: Option<&Approver>, batch: bool) -> Result<()> {
    let name = name(machine);
    if !matches!(approver, Some(Approver::Command { .. })) {
        // A person pastes the code the page shows.
        crate::machine::credentials::require_foreground(batch)?;
    }
    let mut login = Login::start(machine, Provider::Claude, &["claude", "auth", "login"])?;
    // With `BROWSER=true` the login prints only the paste-code page; the
    // localhost-callback one goes to `true`.
    let url = login.next(url_in)?;
    let page = Page {
        provider: Provider::Claude,
        machine: &name,
        url: &url,
        code: None,
    };
    let code = match send(approver, &page) {
        Ok(Sent::Opened(place)) => {
            eprintln!("Approve Claude for {name} {place}, then paste the code it shows:");
            eprint!("> ");
            let mut code = String::new();
            std::io::stdin()
                .lock()
                .read_line(&mut code)
                .context("read the pasted code")?;
            code
        }
        Ok(Sent::Approved(code)) => code,
        Err(error) => {
            login.kill();
            return Err(error);
        }
    };
    let code = code.trim();
    if code.is_empty() {
        login.kill();
        bail!("no code given; {name} was not signed in to Claude");
    }
    login.answer(code)?;
    login.finish(code)?;
    match claude_status(machine)? {
        ClaudeStatus {
            logged_in: true,
            email,
            subscription_type,
        } => {
            let account = email.as_deref().unwrap_or("an account with no email");
            match subscription_type {
                Some(plan) => println!("{name}: Claude signed in as {account} ({plan})"),
                None => println!("{name}: Claude signed in as {account}"),
            }
            Ok(())
        }
        ClaudeStatus { .. } => bail!("Claude's login finished but {name} reports signed out"),
    }
}

/// Device code login: the person, or the approver command, enters the code
/// Codex prints; Codex polls and writes its own login when it is approved.
fn codex(machine: &Machine, approver: Option<&Approver>) -> Result<()> {
    let name = name(machine);
    let mut login = Login::start(
        machine,
        Provider::Codex,
        &["codex", "login", "--device-auth"],
    )?;
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
        Ok(Sent::Approved(_)) => {}
        Err(error) => {
            login.kill();
            return Err(error);
        }
    }
    login.finish(&code)?;
    match codex_status(machine) {
        Status::SignedIn(account) => {
            println!(
                "{name}: Codex signed in ({})",
                account.as_deref().unwrap_or("no account named")
            );
            Ok(())
        }
        Status::SignedOut => bail!("Codex's login finished but {name} reports signed out"),
        Status::Unknown => bail!("Codex's login finished but {name} could not say who it is"),
    }
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
    /// The account the client names: Claude's email and plan, a service login.
    #[serde(deserialize_with = "Option::deserialize")]
    pub account: Option<String>,
}

/// `unknown`: the client could not answer. Shown, never refused on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginState {
    SignedIn,
    SignedOut,
    Unknown,
}

impl LoginState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SignedIn => "signed in",
            Self::SignedOut => "signed out",
            Self::Unknown => "unknown",
        }
    }
}

/// Every provider's login on `machine`. Another Machine answers with its own
/// `lf machine logins --json`, so its clients and store are the ones asked.
pub async fn logins(machine: &Machine) -> Result<Vec<LoginRow>> {
    if !here(machine) {
        let output = crate::machine::credentials::remote_command(
            machine,
            &["machine", "logins", "--json"],
            None,
        )?;
        return serde_json::from_slice(&output)
            .with_context(|| format!("{}'s logins were unreadable", name(machine)));
    }
    let row = |provider, status: Status| {
        let (state, account) = match status {
            Status::SignedIn(account) => (LoginState::SignedIn, account),
            Status::SignedOut => (LoginState::SignedOut, None),
            Status::Unknown => (LoginState::Unknown, None),
        };
        LoginRow {
            provider,
            state,
            account,
        }
    };
    let claude = match claude_status(machine) {
        Ok(ClaudeStatus {
            logged_in: true,
            email,
            subscription_type,
        }) => Status::SignedIn(match (email, subscription_type) {
            (Some(email), Some(plan)) => Some(format!("{email} ({plan})")),
            (email, plan) => email.or(plan),
        }),
        Ok(_) => Status::SignedOut,
        Err(_) => Status::Unknown,
    };
    let mut rows = vec![
        row(Provider::Claude, claude),
        row(Provider::Codex, codex_status(machine)),
    ];
    for service in crate::provider_auth::operations::account_status::report(None)
        .await?
        .accounts
    {
        rows.push(LoginRow {
            provider: service.provider,
            state: match service.state.as_str() {
                "active" => LoginState::SignedIn,
                _ => LoginState::SignedOut,
            },
            account: service.login,
        });
    }
    Ok(rows)
}

pub async fn print_logins(machine: &Machine, json: bool) -> Result<()> {
    let rows = logins(machine).await?;
    if json {
        println!("{}", serde_json::to_string(&rows)?);
        return Ok(());
    }
    for row in rows {
        let line = format!("{:<12} {:<11}", row.provider.as_str(), row.state.as_str());
        match row.account {
            Some(account) => println!("{line}  {account}"),
            None => println!("{}", line.trim_end()),
        }
    }
    Ok(())
}

/// A client's answer to "who is signed in here?".
enum Status {
    SignedIn(Option<String>),
    SignedOut,
    Unknown,
}

/// What `claude auth status --json` reports; it exits 1 when signed out.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStatus {
    pub logged_in: bool,
    pub email: Option<String>,
    pub subscription_type: Option<String>,
}

pub fn claude_status(machine: &Machine) -> Result<ClaudeStatus> {
    let output = command(machine, &["claude", "auth", "status", "--json"])?
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .context("ask Claude for its login")?;
    serde_json::from_slice(&output.stdout)
        .map_err(|_| anyhow!("`claude auth status` on {} was unreadable", name(machine)))
}

/// `codex login status`: exit 0 `Logged in using ChatGPT`, exit 1 `Not logged in`.
fn codex_status(machine: &Machine) -> Status {
    let Ok(output) = command(machine, &["codex", "login", "status"])
        .and_then(|mut command| Ok(command.stdin(Stdio::null()).output()?))
    else {
        return Status::Unknown;
    };
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    match output.status.code() {
        // An API key login reads `Logged in using an API key - sk-…`; only the method crosses.
        Some(0) => Status::SignedIn(said.lines().find_map(|line| {
            let method = line.trim().strip_prefix("Logged in using ")?;
            Some(method.split(" - ").next().unwrap_or(method).to_string())
        })),
        Some(1) if said.contains("Not logged in") => Status::SignedOut,
        _ => Status::Unknown,
    }
}

// ---------------------------------------------------------------------------
// The approver

/// `--chrome-profile` for this run, else `approvers.<provider>`; none opens
/// the default browser.
pub(crate) fn approver(
    provider: Provider,
    chrome_profile: Option<&str>,
) -> Result<Option<Approver>> {
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
    /// The approver command approved it; its first output line, if any.
    Approved(String),
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
            return run_approver(command, page).map(Sent::Approved)
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
fn run_approver(command: &str, page: &Page) -> Result<String> {
    let mut approve = Command::new("sh");
    approve
        .arg("-c")
        .arg(command)
        .env("LF_APPROVE_PROVIDER", page.provider.as_str())
        .env("LF_APPROVE_URL", page.url)
        .env("LF_APPROVE_MACHINE", page.machine)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit());
    match page.code {
        Some(code) => approve.env("LF_APPROVE_CODE", code),
        None => approve.env_remove("LF_APPROVE_CODE"),
    };
    let output = approve
        .output()
        .with_context(|| format!("run the approver for {}", page.provider.display_name()))?;
    if !output.status.success() {
        bail!(
            "the approver for {} exited with {}; {} was not signed in",
            page.provider.display_name(),
            output.status,
            page.machine
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string())
}

// ---------------------------------------------------------------------------
// One provider login process on the target

struct Login {
    child: Child,
    provider: Provider,
    target: String,
    /// Each output line, and whether it came from stderr.
    lines: mpsc::Receiver<(bool, String)>,
    /// The login's recent stderr, for its error line; never shown as a whole.
    errors: Vec<String>,
    started: SystemTime,
}

impl Login {
    fn start(machine: &Machine, provider: Provider, argv: &[&str]) -> Result<Self> {
        let target = name(machine);
        let mut child = command(machine, argv)?
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => not_installed(provider, &target),
                _ => anyhow::Error::from(error).context("start the provider's login"),
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
                    if sender.send((stderr, strip_ansi(&line))).is_err() {
                        break;
                    }
                }
            });
        }
        Ok(Self {
            child,
            provider,
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
                    bail!(
                        "{}'s login on {} printed no approval page",
                        self.provider.display_name(),
                        self.target
                    );
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(self.exited()),
            }
        }
    }

    fn answer(&mut self, code: &str) -> Result<()> {
        let mut stdin = self.child.stdin.take().expect("piped login stdin");
        stdin
            .write_all(format!("{code}\n").as_bytes())
            .with_context(|| format!("send the code to {}", self.target))
    }

    /// Waits for the login to exit successfully, killing it at the deadline.
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
                    "{} was not approved within 15 minutes; {} was not signed in",
                    self.provider.display_name(),
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
        // `sh: claude: command not found` over SSH.
        if exit == Some(127) {
            return not_installed(self.provider, &self.target);
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
        let hint = match self.provider {
            Provider::Codex => "; device code login must be enabled in ChatGPT security settings",
            _ => "",
        };
        anyhow!(
            "{}'s login on {} failed{reason}{hint}",
            self.provider.display_name(),
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

/// `argv` on `machine`: a child here, one SSH exchange without a PTY there.
/// `BROWSER=true` keeps the client from opening a window of its own.
fn command(machine: &Machine, argv: &[&str]) -> Result<Command> {
    if here(machine) {
        let mut command = Command::new(argv[0]);
        command.args(&argv[1..]).env("BROWSER", "true");
        return Ok(command);
    }
    let script = format!(
        "{}; BROWSER=true exec {}",
        crate::machine::ssh::REMOTE_PATH,
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

fn strip_ansi(line: &str) -> String {
    static ANSI: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new("\x1b\\[[0-9;?]*[ -/]*[@-~]").expect("valid ANSI pattern")
    });
    ANSI.replace_all(line, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::strip_ansi;

    #[test]
    fn approval_urls_survive_colored_output() {
        assert_eq!(
            strip_ansi("\x1b[1mvisit:\x1b[0m https://claude.com/cai/oauth/authorize?a=1"),
            "visit: https://claude.com/cai/oauth/authorize?a=1"
        );
    }
}
