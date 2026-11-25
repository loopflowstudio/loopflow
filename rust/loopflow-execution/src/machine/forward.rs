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
    /// `forward.env` names Claude's token but this Machine has no value for it.
    lacks_claude_token: bool,
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
            .field("lacks_claude_token", &self.lacks_claude_token)
            .finish()
    }
}

/// Every value `forward` names, or the first one this Machine cannot supply.
/// Claude's token is a login, not a secret the launch needs: without a value
/// it is left out, and Claude reads signed out where the launch runs.
pub fn read(directory: &Path) -> Result<Forwarded> {
    let config = crate::config::load_config(Some(directory))?
        .unwrap_or_default()
        .forward;
    let mut env = Vec::new();
    let mut lacks_claude_token = false;
    for name in config.env {
        if !valid_name(&name) {
            bail!("forward.env entry {name:?} is not an environment variable name");
        }
        let value = std::env::var(&name)
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| doppler_value(&name, directory));
        match value {
            Some(value) => env.push((name, SecretString::new(value))),
            None if name == crate::machine::login::CLAUDE_TOKEN_ENV => lacks_claude_token = true,
            None => bail!(
                "forward.env names {name}, which has no value on this Machine: export it, \
                 or store it with `doppler secrets set {name}`"
            ),
        }
    }
    Ok(Forwarded {
        env,
        files: read_files(config.files)?,
        lacks_claude_token,
    })
}

fn read_files(entries: Vec<String>) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let home = dirs::home_dir().context("this user has no home directory")?;
    let mut files = Vec::new();
    for entry in entries {
        let relative = home_relative(&entry)?;
        let bytes = std::fs::read(home.join(&relative)).map_err(|error| {
            anyhow!(
                "forward.files names ~/{}, which cannot be read on this Machine: {error}",
                relative.display()
            )
        })?;
        files.push((relative, bytes));
    }
    Ok(files)
}

impl Forwarded {
    /// The environment values, which a request carries itself; the files stay
    /// for [`deliver`].
    pub fn take_secrets(&mut self) -> crate::machine::wire::Secrets {
        crate::machine::wire::Secrets(std::mem::take(&mut self.env))
    }

    /// Whether each launch carries environment variable `name`.
    pub fn carries_env(&self, name: &str) -> bool {
        self.env.iter().any(|(carried, _)| carried == name)
    }

    /// Whether `forward.env` names Claude's token but this Machine has no value.
    pub fn lacks_claude_token(&self) -> bool {
        self.lacks_claude_token
    }
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
    transfer(machine, forwarded, FileDelivery::Keep, notices)
}

#[derive(Clone, Copy)]
enum FileDelivery {
    Keep,
    Replace,
}

/// Explicit refresh reads only the named files, including when a launch token is unavailable.
pub fn refresh_files(directory: &Path, machine: &Machine) -> Result<usize> {
    let config = crate::config::load_config(Some(directory))?.unwrap_or_default();
    let forwarded = Forwarded {
        env: Vec::new(),
        files: read_files(config.forward.files)?,
        lacks_claude_token: false,
    };
    transfer(
        machine,
        &forwarded,
        FileDelivery::Replace,
        &mut std::io::sink(),
    )?;
    Ok(forwarded.files.len())
}

fn transfer(
    machine: &Machine,
    forwarded: &Forwarded,
    delivery: FileDelivery,
    notices: &mut dyn Write,
) -> Result<Option<String>> {
    if forwarded.env.is_empty() && forwarded.files.is_empty() {
        return Ok(None);
    }
    let mut script = vec!["set -e".to_owned(), "umask 077".to_owned()];
    let mode = match delivery {
        FileDelivery::Keep => {
            script.push(format!(
                "[ ! -d \"$HOME/.lf/forward\" ] || find \"$HOME/.lf/forward\" -type f -mmin +{UNREAD_MINUTES} -exec rm -f {{}} +"
            ));
            "keep"
        }
        FileDelivery::Replace => {
            // Verify the registered identity on the connection that replaces the login.
            script.push(crate::machine::ssh::remote_path());
            script.push(format!(
                "{}={} lf machine self </dev/null >/dev/null",
                crate::machine::ssh::EXPECTED_MACHINE_ID_ENV,
                sh_quote(machine.id.as_str())
            ));
            "replace"
        }
    };
    script.push(include_str!("forward.sh").into());
    let mut input = Vec::new();
    for (relative, bytes) in &forwarded.files {
        let path = sh_quote(&relative.to_string_lossy());
        script.push(format!("receive_file {path} {} {mode}", bytes.len()));
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
    use super::{home_relative, read, sourced, valid_name};
    use crate::config::environment::EnvVarGuard;

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

    #[test]
    fn a_missing_claude_token_is_left_out_while_other_missing_names_stop_the_launch() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home = EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        // No `doppler` to ask, and neither name has a value here.
        let _path = EnvVarGuard::set("PATH", home.path().to_str().unwrap());
        let _token = EnvVarGuard::set("CLAUDE_CODE_OAUTH_TOKEN", "");
        let _other = EnvVarGuard::set("LF_TEST_FORWARD_DEPLOY", "");
        let config = home.path().join("config.yaml");
        std::fs::write(&config, "forward:\n  env: [CLAUDE_CODE_OAUTH_TOKEN]\n").unwrap();
        let forwarded = read(home.path()).unwrap();
        assert!(!forwarded.carries_env("CLAUDE_CODE_OAUTH_TOKEN"));
        assert!(forwarded.lacks_claude_token());

        std::fs::write(
            &config,
            "forward:\n  env: [CLAUDE_CODE_OAUTH_TOKEN, LF_TEST_FORWARD_DEPLOY]\n",
        )
        .unwrap();
        let error = read(home.path()).unwrap_err().to_string();
        assert!(
            error.contains("forward.env names LF_TEST_FORWARD_DEPLOY"),
            "{error}"
        );

        let _token = EnvVarGuard::set("CLAUDE_CODE_OAUTH_TOKEN", "sk-ant-oat01-fixture");
        std::fs::write(&config, "forward:\n  env: [CLAUDE_CODE_OAUTH_TOKEN]\n").unwrap();
        let forwarded = read(home.path()).unwrap();
        assert!(forwarded.carries_env("CLAUDE_CODE_OAUTH_TOKEN"));
        assert!(!forwarded.lacks_claude_token());
    }
}
