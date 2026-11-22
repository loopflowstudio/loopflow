//! The direct caller's terminal host and serialized placement.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::process::OsProcess;
use anyhow::{bail, Context, Result};
use serde_json::Value;

pub const HOST_TIMEOUT: Duration = Duration::from_secs(2);

fn set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

const CMUX_WORKSPACE: &str = "CMUX_WORKSPACE_ID";
const CMUX_SOCKET: &str = "CMUX_SOCKET_PATH";
const HERDR_ENV: &str = "HERDR_ENV";
const HERDR_SOCKET: &str = "HERDR_SOCKET_PATH";
const HERDR_WORKSPACE: &str = "HERDR_WORKSPACE_ID";
const HERDR_PANE: &str = "HERDR_PANE_ID";
const TMUX: &str = "TMUX";
const TMUX_PANE: &str = "TMUX_PANE";
/// Terminal-host context a direct caller carries. The link receiver removes
/// it: an OS receiver's environment belongs to its first launch, not the click.
pub(crate) const HOST_CONTEXT: &[&str] = &[
    "REX_BIN_DIR",
    "REX_SERVER",
    "REX_SESSION",
    "REX_BLOCK",
    CMUX_WORKSPACE,
    CMUX_SOCKET,
    HERDR_ENV,
    HERDR_SOCKET,
    HERDR_WORKSPACE,
    HERDR_PANE,
    TMUX,
    TMUX_PANE,
    "LF_DESKTOP",
];

pub fn in_cmux() -> bool {
    set(CMUX_WORKSPACE) && set(CMUX_SOCKET)
}

pub fn in_rex() -> bool {
    set("REX_SESSION") || set("REX_BLOCK")
}

pub fn in_herdr() -> bool {
    std::env::var(HERDR_ENV).as_deref() == Ok("1") && set(HERDR_SOCKET) && set(HERDR_PANE)
}

pub fn in_tmux() -> bool {
    set(TMUX) && set(TMUX_PANE)
}

/// The host workspace this direct process runs in, recorded as its attached
/// terminal placement (host endpoint and view identity)
/// so a later opening can focus that conversation instead of starting another client.
pub(crate) fn host_placement() -> Option<String> {
    if in_rex() {
        let location = RexLocation::current().ok()?;
        return Some(format!("rex:{}", serde_json::to_string(&location).ok()?));
    }
    if in_herdr() {
        return Some(format!(
            "herdr:{}",
            serde_json::to_string(&HerdrLocation::current()?).ok()?
        ));
    }
    if in_cmux() {
        return Some(format!(
            "cmux:{}",
            serde_json::to_string(&CmuxLocation::current()?).ok()?
        ));
    }
    Some(format!(
        "tmux:{}",
        serde_json::to_string(&TmuxLocation::current()?).ok()?
    ))
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct HerdrLocation {
    pub socket: String,
    pub pane: String,
}

impl HerdrLocation {
    pub fn current() -> Option<Self> {
        Some(Self {
            socket: std::env::var("HERDR_SOCKET_PATH").ok()?,
            pane: std::env::var("HERDR_PANE_ID").ok()?,
        })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CmuxLocation {
    pub socket: String,
    pub workspace: String,
}

impl CmuxLocation {
    pub fn current() -> Option<Self> {
        Some(Self {
            socket: std::env::var("CMUX_SOCKET_PATH").ok()?,
            workspace: std::env::var("CMUX_WORKSPACE_ID").ok()?,
        })
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TmuxLocation {
    pub socket: String,
    pub pane: String,
}

impl TmuxLocation {
    pub fn current() -> Option<Self> {
        let tmux = std::env::var("TMUX").ok()?;
        Some(Self {
            socket: tmux.rsplitn(3, ',').nth(2)?.into(),
            pane: std::env::var("TMUX_PANE").ok()?,
        })
    }
}

fn rex_bin_dir() -> Option<PathBuf> {
    std::env::var_os("REX_BIN_DIR")
        .filter(|dir| !dir.is_empty())
        .map(PathBuf::from)
}

pub fn rex_command(viewer_bin_dir: Option<&Path>) -> Command {
    let ambient = rex_bin_dir();
    let program = viewer_bin_dir
        .into_iter()
        .chain(ambient.as_deref())
        .map(|dir| dir.join("rex"))
        .find(|path| path.is_file())
        .unwrap_or_else(|| "rex".into());
    Command::new(program)
}

pub fn rex_endpoint(server: Option<&str>, viewer_bin_dir: Option<&Path>) -> Result<String> {
    let mut command = rex_command(viewer_bin_dir);
    for name in ["REX_SERVER", "REX_SESSION", "REX_BLOCK"] {
        command.env_remove(name);
    }
    if let Some(server) = server {
        command.args(["-S", server]);
    }
    command.args(["--autostart=false", "--timeout", "2s", "whoami", "--json"]);
    let output = run_host(command, "Rex")?;
    let who: Value = serde_json::from_str(&output)?;
    let server = who["endpoint"]
        .as_str()
        .context("Rex did not identify its server")?;
    // Unix peer authentication identifies this Machine's account. A remotely
    // selected default is never accepted as the local Task execution host.
    anyhow::ensure!(server.starts_with("unix://") && who["principal"]["transport"] == "unix", "Rex is connected to another Machine; run lf open on the Task's recorded Machine with its local Rex server");
    Ok(server.into())
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RexLocation {
    /// Rex supplies this to the viewer. A detached agent may have a different PATH.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bin_dir: Option<PathBuf>,
    pub server: String,
    pub session: String,
    pub block: String,
}

impl RexLocation {
    pub fn current() -> Result<Self> {
        let server = match std::env::var("REX_SERVER").ok() {
            Some(server) => server,
            None => caller_server()?.context(
                "Rex caller server is unavailable from its process ancestry; select it with --server",
            )?,
        };
        Self {
            bin_dir: rex_bin_dir(),
            server,
            session: std::env::var("REX_SESSION").context("Rex caller session is missing")?,
            block: std::env::var("REX_BLOCK").context("Rex caller block is missing")?,
        }
        .resolve()
    }

    pub fn resolve(mut self) -> Result<Self> {
        self.server = rex_endpoint(Some(&self.server), self.bin_dir.as_deref())?;
        Ok(self)
    }
}

/// Native Rex shells descend from the serving process, even through login and
/// tool shells. Its explicit listener is destination evidence; a running-app or
/// socket inventory is not. Current conversation placement is resolved first.
fn caller_server() -> Result<Option<String>> {
    let mut pid = std::process::id();
    let mut seen = BTreeSet::new();
    while pid > 1 && seen.insert(pid) {
        let Some(process) = OsProcess::read(pid)? else {
            break;
        };
        if OsProcess::executable(pid)
            .is_some_and(|path| path.file_name().is_some_and(|name| name == "rex"))
        {
            let args = OsProcess::arguments(pid)?
                .context("Rex caller server arguments are unavailable")?;
            if args.windows(2).any(|pair| pair == ["server", "run"]) {
                return Ok(listening_endpoint(&args));
            }
        }
        pid = process.ppid;
    }
    Ok(None)
}

pub(crate) fn listening_endpoint(args: &[String]) -> Option<String> {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        if arg == "--listen" {
            return args.next().cloned();
        }
        if let Some(endpoint) = arg.strip_prefix("--listen=") {
            return Some(endpoint.into());
        }
    }
    None
}

/// Run one host request with a finite timeout and return its stdout.
pub fn run_host(mut command: Command, host: &str) -> Result<String> {
    // Native inventories can exceed a pipe buffer. Spool while the child runs
    // so waiting for exit cannot deadlock behind an unread stdout/stderr pipe.
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()
        .with_context(|| format!("cannot run {host}"))?;
    let (status, timed_out) =
        crate::os_process::wait_for_exit(&mut child, Some(HOST_TIMEOUT), || {})?;
    if timed_out {
        let _ = child.kill();
        let _ = child.wait();
        bail!("{host} did not answer within {}s", HOST_TIMEOUT.as_secs());
    }
    use std::io::Seek;
    stdout.rewind()?;
    stderr.rewind()?;
    let stdout = std::io::read_to_string(stdout)?;
    let stderr = std::io::read_to_string(stderr)?;
    if status.success() {
        Ok(stdout)
    } else {
        let detail = stderr.trim();
        anyhow::ensure!(!detail.is_empty(), "{host} exited with {status}");
        bail!("{host} refused: {detail}")
    }
}

#[cfg(test)]
mod tests {
    use super::listening_endpoint;
    #[test]
    fn caller_listener_preserves_argument_boundaries() {
        for args in [
            vec![
                "rex",
                "server",
                "run",
                "--listen",
                "unix:///Task servers/rex.sock",
            ],
            vec![
                "rex",
                "server",
                "run",
                "--listen=unix:///Task servers/rex.sock",
            ],
        ] {
            assert_eq!(
                listening_endpoint(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()),
                Some("unix:///Task servers/rex.sock".into())
            );
        }
        assert_eq!(
            listening_endpoint(&["--config".into(), "/rex.lua".into()]),
            None
        );
    }
}
