//! Bounded request/reply transport to an identified Machine.
use anyhow::Context;
use std::io::IsTerminal;
use std::process::{Command, Stdio};

pub const EXPECTED_MACHINE_ID_ENV: &str = "LF_EXPECTED_MACHINE_ID";
// Non-interactive SSH skips shell profiles, including the npm prefix.
pub const REMOTE_PATH: &str = "export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; if command -v npm >/dev/null 2>&1; then PATH=\"$PATH:$(npm config get prefix 2>/dev/null)/bin\"; fi";

pub fn build_preamble(cmd: &[String], extra_env: &[(&str, &str)]) -> String {
    let mut lines = vec![REMOTE_PATH.to_string()];
    for (name, value) in extra_env {
        lines.push(format!("export {name}={}", sh_quote(value)));
    }
    lines.push(format!(
        "exec {}",
        cmd.iter()
            .map(|arg| sh_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    lines.join("\n") + "\n"
}

/// POSIX single-quote escaping: wrap in `'…'`, and render any embedded single
/// quote as `'\''`. Safe for arbitrary bytes including secrets.
pub fn sh_quote(value: &str) -> String {
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

/// One bounded request through the ordinary SSH envelope, with `input` on its
/// stdin. No probes, credentials, provider launch or local fallback; callers
/// validate the request-bound reply. A request carrying input (a moved
/// conversation) may check out its branch there, so it waits longer.
pub async fn request(
    machine: &loopflow_model::durable::Machine,
    args: &[String],
    input: Option<&[u8]>,
) -> anyhow::Result<Vec<u8>> {
    use tokio::io::AsyncWriteExt;
    let cmd = std::iter::once("lf".to_string())
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();
    let preamble = build_preamble(&cmd, &[(EXPECTED_MACHINE_ID_ENV, machine.id.as_str())]);
    let mut command = tokio::process::Command::new("ssh");
    command
        .args(crate::machine_route::bounded_ssh_args(
            &machine.route,
            false,
        )?)
        .arg(preamble)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let bound = std::time::Duration::from_secs(if input.is_some() { 300 } else { 20 });
    let exchange = async {
        let mut child = command.spawn()?;
        if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
            stdin.write_all(input).await?;
        }
        child.wait_with_output().await
    };
    let output = tokio::time::timeout(bound, exchange)
        .await
        .context("Machine command unavailable: peer timed out")??;
    anyhow::ensure!(
        output.status.success(),
        "Machine command unavailable: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(output.stdout)
}

/// One typed operation, answered by `lf --repo-id <repository> server stdio`
/// on `machine`: `hello`, then one `request`, on its stdin. Returns the
/// response's result; an error it answered keeps its code. Its stderr lines
/// become this request's diagnostics. An `lf` too old for the entry fails
/// naming its version and the update command; nothing runs here instead.
pub fn call(
    machine: &loopflow_model::durable::Machine,
    repository: &loopflow_model::durable::RepositoryId,
    environment: &[(String, String)],
    operation: &impl serde::Serialize,
) -> anyhow::Result<serde_json::Value> {
    use super::wire::{ClientMessage, Hello, ServerMessage, MIN_REVISION, REVISION};
    use crate::engine::{Error, ErrorCode};
    let messages: [ClientMessage; 2] = [
        ClientMessage::Hello(Hello {
            revision: REVISION,
            min_revision: MIN_REVISION,
            client: super::client::identity("machine", "lf"),
        }),
        ClientMessage::Request {
            id: "1".into(),
            operation: serde_json::to_value(operation)?,
            select: None,
        },
    ];
    let mut input = Vec::new();
    for message in &messages {
        serde_json::to_writer(&mut input, message)?;
        input.push(b'\n');
    }
    let command = ["lf", "--repo-id", repository.as_str(), "server", "stdio"].map(str::to_owned);
    let environment: Vec<_> = environment
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let (mut output, mut error) = (Vec::new(), Vec::new());
    let ran = run_ssh_io(
        &machine.route,
        false,
        &build_preamble(&command, &environment),
        true,
        &mut crate::command::CommandIo {
            input: &mut input.as_slice(),
            output: &mut output,
            error: &mut error,
        },
    );
    let said = String::from_utf8_lossy(&error);
    let answers: Vec<ServerMessage> = output
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice)
        .collect::<Result<_, _>>()
        .unwrap_or_default();
    let failed = |failure: anyhow::Error| match said.trim() {
        "" => failure,
        said => failure.context(format!("the destination said: {said}")),
    };
    match answers.as_slice() {
        [ServerMessage::Welcome(_), answer, ..] => {
            for line in said.lines() {
                crate::journal::request::diagnostic(format_args!("{line}"));
            }
            match answer {
                ServerMessage::Response { result, .. } => Ok(result.clone()),
                ServerMessage::Error {
                    code,
                    message,
                    data,
                    ..
                } => Err(Error {
                    code: *code,
                    message: message.clone(),
                    data: data.clone(),
                }
                .into()),
                other => anyhow::bail!("unexpected answer from {}: {other:?}", machine.route),
            }
        }
        [ServerMessage::Error {
            code,
            message,
            data,
            ..
        }, ..] => Err(Error {
            code: *code,
            message: message.clone(),
            data: data.clone(),
        }
        .into()),
        [ServerMessage::Welcome(_)] => Err(failed(
            Error::outcome_unknown(&anyhow::anyhow!(
                "{} closed the connection before answering",
                machine.route
            ))
            .into(),
        )),
        _ => match ran {
            // Clap's usage error: this `lf` has no `server stdio`.
            Err(exit)
                if exit
                    .downcast_ref::<crate::process::CommandExit>()
                    .is_some_and(|exit| exit.0 == 2) =>
            {
                Err(Error::new(ErrorCode::Incompatible, outdated(machine)).into())
            }
            Err(failure) => Err(failed(failure)),
            Ok(()) => Err(failed(anyhow::anyhow!(
                "{} did not answer the operation",
                machine.route
            ))),
        },
    }
}

/// `mini runs lf 0.15.2, which cannot take this operation; update it with …`.
fn outdated(machine: &loopflow_model::durable::Machine) -> String {
    let name = machine.label.as_deref().unwrap_or(machine.route.as_str());
    let update = super::connection::update_command(&machine.route);
    let version = tokio::runtime::Runtime::new()
        .ok()
        .and_then(|runtime| {
            runtime
                .block_on(super::connection::probe(&machine.route, false))
                .ok()
        })
        .map(|probe| probe.version);
    match version {
        Some(version) => format!(
            "{name} runs lf {version}, which cannot take this operation; update it with `{update}`"
        ),
        None => {
            format!("{name} runs an lf that cannot take this operation; update it with `{update}`")
        }
    }
}

pub fn run_ssh_io(
    dest: &str,
    forward_agent: bool,
    preamble: &str,
    reads_input: bool,
    io: &mut crate::command::CommandIo<'_>,
) -> anyhow::Result<()> {
    use std::io::Write;
    let args = ssh_argv(dest, forward_agent, preamble)?;
    let mut input = Vec::new();
    if reads_input {
        io.input.read_to_end(&mut input)?;
    }
    let mut child = Command::new(&args[0])
        .args(&args[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("piped SSH stdin");
    // Drain output while sending large drafts: either pipe can fill first.
    let output = std::thread::scope(|scope| {
        let sending = scope.spawn(move || stdin.write_all(&input));
        let output = child.wait_with_output()?;
        let sent = sending.join().expect("SSH input writer panicked");
        if output.status.success() {
            sent?;
        }
        Ok::<_, std::io::Error>(output)
    })?;
    io.output.write_all(&output.stdout)?;
    io.error.write_all(&output.stderr)?;
    command_result(dest, output.status.code())
}

/// Classify an ssh exit code. `255` is ssh's reserved transport-error code;
/// `None` means death by signal — both are connection-phase failures, distinct
/// from a real remote command code we must propagate.
pub fn command_result(dest: &str, code: Option<i32>) -> anyhow::Result<()> {
    match code {
        Some(0) => Ok(()),
        Some(255) | None => Err(crate::machine::connection::transport_failure(
            dest,
            "SSH transport closed; see the SSH error above",
        )),
        Some(code) => Err(crate::process::CommandExit(
            u8::try_from(code).expect("SSH command exit status fits a byte"),
        )
        .into()),
    }
}

/// Carry the script as an argument: stdin belongs to file drafts or terminal input.
pub fn ssh_argv(dest: &str, forward_agent: bool, preamble: &str) -> anyhow::Result<Vec<String>> {
    let args = crate::machine_route::bounded_ssh_args(dest, forward_agent)?;
    let mut argv = vec!["ssh".into()];
    argv.extend(args);
    argv.push(preamble.into());
    Ok(argv)
}

pub fn run_ssh(dest: &str, forward_agent: bool, preamble: &str) -> anyhow::Result<()> {
    let mut argv = ssh_argv(dest, forward_agent, preamble)?;
    if std::io::stdin().is_terminal() {
        for arg in &mut argv {
            if arg == "-T" {
                *arg = "-t".into();
            }
        }
    }
    let status = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("ssh did not complete")?;
    command_result(dest, status.code())
}

#[cfg(test)]
mod tests {
    use super::{build_preamble, command_result};
    #[test]
    fn repository_addressed_transport_preserves_arguments_in_the_home_directory() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let lf = bin.join("lf");
        std::fs::write(&lf, "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\"\n").unwrap();
        std::fs::set_permissions(&lf, std::fs::Permissions::from_mode(0o755)).unwrap();
        let script = build_preamble(
            &[
                "lf".into(),
                "--repo-id".into(),
                "repo_exact".into(),
                "--task".into(),
                "task_exact".into(),
                ":".into(),
                "literal 'draft'; $(false)".into(),
            ],
            &[("LF_EXPECTED_MACHINE_ID", "home_fixture")],
        );
        let output = std::process::Command::new("/bin/bash")
            .args(["-c", &script])
            .env_clear()
            .env("HOME", dir.path())
            .env("PATH", "/usr/bin:/bin")
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let lines = text.lines().collect::<Vec<_>>();
        assert_eq!(
            lines[0],
            dir.path().canonicalize().unwrap().to_str().unwrap()
        );
        assert_eq!(
            &lines[1..],
            &[
                "--repo-id",
                "repo_exact",
                "--task",
                "task_exact",
                ":",
                "literal 'draft'; $(false)"
            ]
        );
    }

    #[test]
    fn ssh_args_bound_the_connection() {
        let args = crate::machine_route::bounded_ssh_args("jack@mini-heart", false).unwrap();
        // Primary hang killer: never block on an interactive prompt.
        assert!(args.iter().any(|a| a == "BatchMode=yes"));
        // Connect handshake and stalled-session bounds.
        assert!(args.iter().any(|a| a == "ConnectTimeout=10"));
        assert!(args.iter().any(|a| a == "ServerAliveInterval=10"));
        assert!(args.iter().any(|a| a == "ServerAliveCountMax=3"));
        assert_eq!(args.last().unwrap(), "jack@mini-heart");
        // Agent forwarding stays opt-in; OpenSSH parses destination ports.
        assert!(!args.iter().any(|a| a == "-A"));
        assert!(!args.iter().any(|a| a == "-p"));
    }

    #[test]
    fn ssh_args_opt_in_agent_forwarding() {
        let args = crate::machine_route::bounded_ssh_args("host", true).unwrap();
        assert_eq!(args.first().unwrap(), "-A");
    }

    #[test]
    fn command_result_preserves_remote_exit_codes_and_reports_transport_failure() {
        assert!(command_result("mini", Some(0)).is_ok());
        for code in [Some(255), None] {
            let error = command_result("mini", code).unwrap_err();
            assert!(error
                .downcast_ref::<crate::process::CommandExit>()
                .is_none());
            assert!(error.to_string().contains("unreachable"));
            assert!(error.to_string().contains("mini"));
        }
        for code in [1, 42, 254] {
            let error = command_result("mini", Some(code)).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<crate::process::CommandExit>()
                    .unwrap()
                    .0,
                code as u8
            );
        }
    }
}
