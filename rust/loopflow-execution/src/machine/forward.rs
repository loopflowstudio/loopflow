//! Environment variables and home files a remote launch carries from this
//! Machine, as personal `forward` configuration names them. lf does not know
//! what they are: it moves only what the person named, never in argv, logs,
//! the store or Session records.
use anyhow::{anyhow, bail, Context, Result};
use loopflow_model::durable::Machine;
use secrecy::{ExposeSecret, SecretString};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use crate::machine::ssh::sh_quote;

/// The values a launch forwards, read on this Machine. Debug shows names only.
pub struct Forwarded {
    env: Vec<(String, SecretString)>,
    files: Vec<(PathBuf, Vec<u8>)>,
}

impl std::fmt::Debug for Forwarded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Forwarded")
            .field(
                "env",
                &self.env.iter().map(|(name, _)| name).collect::<Vec<_>>(),
            )
            .field(
                "files",
                &self.files.iter().map(|(path, _)| path).collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// Every value `forward` names, or the first one this Machine cannot supply.
pub fn read(directory: &Path) -> Result<Forwarded> {
    let config = crate::config::load_config(Some(directory))?
        .unwrap_or_default()
        .forward;
    let mut env = Vec::new();
    for name in config.env {
        if !valid_name(&name) {
            bail!("forward.env entry {name:?} is not an environment variable name");
        }
        let value = std::env::var(&name)
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| doppler_value(&name, directory))
            .ok_or_else(|| {
                anyhow!(
                    "forward.env names {name}, which has no value on this Machine: export it, \
                     or store it with `doppler secrets set {name}`"
                )
            })?;
        env.push((name, SecretString::new(value)));
    }
    let home = dirs::home_dir().context("this user has no home directory")?;
    let mut files = Vec::new();
    for entry in config.files {
        let relative = home_relative(&entry)?;
        let bytes = std::fs::read(home.join(&relative)).map_err(|error| {
            anyhow!(
                "forward.files names ~/{}, which cannot be read on this Machine: {error}",
                relative.display()
            )
        })?;
        files.push((relative, bytes));
    }
    Ok(Forwarded { env, files })
}

/// What Claude and Codex sign in with: a setup or access token in the
/// environment, or the login file under the home.
const AGENT_LOGIN_ENV: [&str; 2] = ["CLAUDE_CODE_OAUTH_TOKEN", "CODEX_ACCESS_TOKEN"];
const AGENT_LOGIN_FILES: [&str; 2] = [".claude/.credentials.json", ".codex/auth.json"];

impl Forwarded {
    /// The environment values, which a request carries itself; the files stay
    /// for [`deliver`].
    pub fn take_secrets(&mut self) -> crate::machine::wire::Secrets {
        crate::machine::wire::Secrets(std::mem::take(&mut self.env))
    }

    fn carries_agent_login(&self) -> bool {
        self.env
            .iter()
            .any(|(name, _)| AGENT_LOGIN_ENV.contains(&name.as_str()))
            || self
                .files
                .iter()
                .any(|(path, _)| AGENT_LOGIN_FILES.iter().any(|file| path == Path::new(file)))
    }
}

/// `read` for an on-demand rental, which starts with no login of its own: a
/// `forward` that carries no Claude or Codex login stops it before anything
/// is rented.
pub fn read_for_rental(directory: &Path) -> Result<Forwarded> {
    let forwarded = read(directory)?;
    if !forwarded.carries_agent_login() {
        bail!(
            "an on-demand rental has no Claude or Codex login, and `forward` names none to carry; \
             after `claude setup-token`, `doppler secrets set CLAUDE_CODE_OAUTH_TOKEN` and \
             `lf set --global forward.env '[CLAUDE_CODE_OAUTH_TOKEN]'`, \
             or `lf set --global forward.files '[~/.codex/auth.json]'`"
        );
    }
    Ok(forwarded)
}

/// Forwarded environment files older than this were never read by their
/// launch (its SSH failed); the next delivery removes them.
const UNREAD_MINUTES: u32 = 10;

/// Put the named files and environment on `machine` over one SSH exchange whose
/// script carries only names and lengths; the values travel on its stdin. A
/// destination that already has a named file keeps its own; when its copy
/// differs, `notices` says so. Returns the one-time file under the
/// destination's home that holds the environment, which a command there
/// reads with [`sourced`].
pub fn deliver(
    machine: &Machine,
    forwarded: &Forwarded,
    notices: &mut dyn Write,
) -> Result<Option<String>> {
    if forwarded.env.is_empty() && forwarded.files.is_empty() {
        return Ok(None);
    }
    let mut script = vec![
        "set -e".to_owned(),
        "umask 077".to_owned(),
        format!(
            "[ ! -d \"$HOME/.lf/forward\" ] || find \"$HOME/.lf/forward\" -type f -mmin +{UNREAD_MINUTES} -exec rm -f {{}} +"
        ),
    ];
    let mut input = Vec::new();
    for (relative, bytes) in &forwarded.files {
        let path = remote_path(&relative.to_string_lossy());
        let shown = sh_quote(&format!("~/{}", relative.display()));
        let count = bytes.len();
        script.push(format!(
            "p={path}; t=\"$p.lf-forward\"; d=$(dirname \"$p\"); [ -d \"$d\" ] || mkdir -p \"$d\"; \
             dd bs=1 count={count} of=\"$t\" 2>/dev/null; \
             if [ -e \"$p\" ]; then cmp -s \"$t\" \"$p\" || echo kept {shown}; rm -f \"$t\"; \
             else chmod 600 \"$t\"; mv \"$t\" \"$p\"; fi"
        ));
        input.extend_from_slice(bytes);
    }
    let env_file = (!forwarded.env.is_empty()).then(|| {
        let block: String = forwarded
            .env
            .iter()
            .map(|(name, value)| format!("export {name}={}\n", sh_quote(value.expose_secret())))
            .collect();
        let file = format!(".lf/forward/{}", uuid::Uuid::new_v4().simple());
        script.push(format!(
            "[ -d \"$HOME/.lf/forward\" ] || mkdir -p \"$HOME/.lf/forward\"; \
             dd bs=1 count={} of={} 2>/dev/null",
            block.len(),
            remote_path(&file)
        ));
        input.extend_from_slice(block.as_bytes());
        file
    });
    let mut child = Command::new("ssh")
        .args(crate::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(script.join("\n"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("could not start ssh to forward named values")?;
    let mut stdin = child.stdin.take().expect("piped SSH stdin");
    let output = std::thread::scope(|scope| {
        let sending = scope.spawn(move || stdin.write_all(&input));
        let output = child.wait_with_output()?;
        let sent = sending.join().expect("forward writer panicked");
        if output.status.success() {
            sent?;
        }
        Ok::<_, std::io::Error>(output)
    })?;
    let name = machine.name();
    if !output.status.success() {
        // The script echoes names only, so its error output is safe to show.
        bail!(
            "could not forward named values to {name}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if let Some(path) = line.strip_prefix("kept ") {
            writeln!(notices, "{name} keeps its own {path}")?;
        }
    }
    Ok(env_file)
}

/// `preamble`, after reading the delivered environment and removing its file.
pub fn sourced(env_file: &str, preamble: &str) -> String {
    format!(
        "f={}; . \"$f\"; rm -f \"$f\"\n{preamble}",
        remote_path(env_file)
    )
}

fn remote_path(relative: &str) -> String {
    format!("\"$HOME\"/{}", sh_quote(relative))
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|c| c == '_' || c.is_ascii_alphanumeric())
}

/// `~/.codex/auth.json` and `.codex/auth.json` both name a file under the home.
fn home_relative(entry: &str) -> Result<PathBuf> {
    let path = Path::new(entry.strip_prefix("~/").unwrap_or(entry));
    if path.as_os_str().is_empty()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("forward.files entry {entry:?} must be a path under the home directory");
    }
    Ok(path.to_path_buf())
}

/// Doppler's value for `name` in the directory's configured project, if any.
fn doppler_value(name: &str, directory: &Path) -> Option<String> {
    let output = Command::new("doppler")
        .args(["secrets", "get", name, "--plain"])
        .current_dir(directory)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.strip_suffix('\n').unwrap_or(&value);
    (output.status.success() && !value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{home_relative, sourced, valid_name, Forwarded};
    use secrecy::SecretString;
    use std::path::PathBuf;

    #[test]
    fn a_rental_needs_a_forwarded_claude_or_codex_login() {
        let forwarded = |env: &[&str], files: &[&str]| Forwarded {
            env: env
                .iter()
                .map(|name| (name.to_string(), SecretString::new("value".into())))
                .collect(),
            files: files
                .iter()
                .map(|path| (PathBuf::from(path), Vec::new()))
                .collect(),
        };
        assert!(!forwarded(&[], &[]).carries_agent_login());
        assert!(
            !forwarded(&["GH_TOKEN", "ANTHROPIC_API_KEY"], &[".ssh/config"]).carries_agent_login()
        );
        assert!(forwarded(&["CLAUDE_CODE_OAUTH_TOKEN"], &[]).carries_agent_login());
        assert!(forwarded(&[], &[".codex/auth.json"]).carries_agent_login());
        assert!(forwarded(&[], &[".claude/.credentials.json"]).carries_agent_login());
    }

    #[test]
    fn names_and_paths_are_checked_before_anything_moves() {
        assert!(valid_name("CLAUDE_CODE_OAUTH_TOKEN"));
        assert!(!valid_name("1TOKEN"));
        assert!(!valid_name("A B"));
        assert_eq!(
            home_relative("~/.codex/auth.json").unwrap(),
            std::path::Path::new(".codex/auth.json")
        );
        for entry in ["/etc/passwd", "../x", "~/a/../../b", ""] {
            assert!(home_relative(entry).is_err(), "{entry}");
        }
    }

    #[test]
    fn a_forwarded_environment_is_read_and_removed_before_the_launch() {
        assert_eq!(
            sourced(".lf/forward/abc", "exec lf\n"),
            "f=\"$HOME\"/'.lf/forward/abc'; . \"$f\"; rm -f \"$f\"\nexec lf\n"
        );
    }
}
