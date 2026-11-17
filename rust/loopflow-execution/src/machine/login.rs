//! `lf machine login`: each provider's own sign-in, run on the Machine that
//! keeps the login and approved in this Machine's browser.
//!
//! For Claude the target runs `claude auth login` unmodified, locally as a
//! child or behind one SSH exchange without a PTY. lf reads its approval URL,
//! opens it here and writes the code the page shows to the login's stdin. The
//! code is useless without the PKCE verifier inside that `claude` process, and
//! lf stores nothing: the client writes and refreshes its own login.
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

use crate::profile::LocalChromeProfile;
use crate::provider_auth::Provider;
use loopflow_model::durable::Machine;

/// How long a sign-in may wait for its approval: Codex's device code lifetime,
/// which Claude shares so both fail the same way.
const APPROVAL_DEADLINE: Duration = Duration::from_secs(15 * 60);
/// How long the login may take to print its approval URL, SSH included.
const URL_DEADLINE: Duration = Duration::from_secs(60);

/// Sign `machine` in to `provider`. A Machine whose route is `local` is this one.
pub async fn login(
    machine: &Machine,
    provider: Provider,
    chrome_profile: Option<&str>,
    api_key: bool,
    batch: bool,
) -> Result<()> {
    match provider {
        Provider::Claude => claude(machine, chrome_profile, batch),
        Provider::Codex => bail!(
            "Codex sign-in is not built yet; run `codex login --device-auth` on {}",
            name(machine)
        ),
        _ if here(machine) => {
            crate::provider_auth::operations::connect_target(
                provider.as_str(),
                chrome_profile,
                api_key,
            )
            .await
        }
        _ => crate::machine::credentials::connect(machine, provider, chrome_profile, batch).await,
    }
}

fn claude(machine: &Machine, chrome_profile: Option<&str>, batch: bool) -> Result<()> {
    let name = name(machine);
    // A person pastes the code the page shows.
    crate::machine::credentials::require_foreground(batch)?;
    let approver = Approver::new(chrome_profile)?;
    let mut login = Login::start(machine, Provider::Claude, &["claude", "auth", "login"])?;
    let url = login.approval_url()?;
    let place = approver.open(&url);
    eprintln!("Approve Claude for {name} {place}, then paste the code it shows:");
    eprint!("> ");
    let mut code = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut code)
        .context("read the pasted code")?;
    let code = code.trim();
    if code.is_empty() {
        login.kill();
        bail!("no code pasted; {name} was not signed in to Claude");
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

// ---------------------------------------------------------------------------
// The approver

/// Where an approval page opens.
enum Approver {
    Browser,
    Chrome(LocalChromeProfile),
}

impl Approver {
    fn new(chrome_profile: Option<&str>) -> Result<Self> {
        Ok(match chrome_profile {
            Some(requested) => Self::Chrome(
                crate::profile::resolve_local_chrome_profile(requested)
                    .map_err(anyhow::Error::msg)?,
            ),
            None => Self::Browser,
        })
    }

    /// Opens `url` and says where; a page that could not open is printed instead.
    fn open(&self, url: &str) -> String {
        let (opened, place) = match self {
            Self::Browser => (
                crate::platform::open_url_checked(url).map_err(anyhow::Error::from),
                "in your browser".to_string(),
            ),
            Self::Chrome(profile) => (
                crate::provider_auth::operations::open_chrome_profile(profile, url),
                format!("in Chrome ({})", profile.name),
            ),
        };
        match opened {
            Ok(()) => place,
            Err(error) => {
                eprintln!("Could not open the approval page ({error}); open it yourself:\n  {url}");
                "there".to_string()
            }
        }
    }
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

    /// The first URL the login prints. With `BROWSER=true` it only prints the
    /// paste-code page; the localhost-callback one goes to `true`.
    fn approval_url(&mut self) -> Result<String> {
        let deadline = SystemTime::now() + URL_DEADLINE;
        loop {
            let left = deadline
                .duration_since(SystemTime::now())
                .unwrap_or_default();
            match self.lines.recv_timeout(left) {
                Ok((stderr, line)) => {
                    if let Some(start) = line.find("https://") {
                        let url = line[start..].split_whitespace().next().unwrap_or_default();
                        return Ok(url.to_string());
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
        anyhow!(
            "{}'s login on {} failed{reason}",
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
