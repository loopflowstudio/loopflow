//! The exec door's engine — validate an `lf` argv against the clap command
//! tree, then exec `lf` directly (no shell) and capture the result.
//!
//! Shared, state-free, host-neutral: both the lfd machine face (`/v0/exec`)
//! and the per-wave listener (the in-wave subagent → outwave backdoor) run
//! their door through these two functions. The door is a dumb pipe — it does
//! not interpret verb semantics. It (1) checks the argv *parses* as a valid
//! `lf` command (the "errors if it doesn't compile" gate) and (2) execs it,
//! capturing stdout/stderr/exit. Authority is the host's business (a
//! capability/resident token); this engine trusts its caller.
//!
//! Security: `Command::new(lf).args(argv)` passes argv straight to the binary
//! with **no shell**, so there is no shell-injection surface. The door execs
//! exactly what an authorized caller could run as `lf` themselves.

use crate::lfd::executor::resolve_lf_binary;

/// The outcome of an exec: the process exit code plus its captured streams.
/// A non-zero `exit_code` is a *successful* door call reporting a failed `lf`
/// run — distinct from the door refusing to exec (invalid argv → the caller
/// gets a 400 and this is never produced).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LfExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Validate that `argv` parses as an `lf` command. Returns the clap error
/// rendered for the caller on failure — the door refuses to exec argv that
/// would not parse. `argv` is the command line *after* the binary name
/// (e.g. `["next", "--create-pr"]`).
pub(crate) fn validate_lf_argv(argv: &[String]) -> Result<(), String> {
    use clap::error::ErrorKind;
    use clap::Parser;
    // Bare `lf` parses (it launches an interactive session), but the door has
    // nothing to run without a subcommand — refuse it explicitly.
    if argv.is_empty() {
        return Err("argv is empty: no lf command to run".to_string());
    }
    let full = std::iter::once("lf".to_string()).chain(argv.iter().cloned());
    match crate::lf::Cli::try_parse_from(full) {
        Ok(_) => Ok(()),
        // `--help`/`--version` are surfaced by clap as "errors" from
        // `try_parse`, but on the CLI `lf` prints them and exits 0. Let them
        // through so the door execs `lf` and the caller gets the real
        // help/version output — same as running `lf` themselves.
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::DisplayHelp
                    | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                    | ErrorKind::DisplayVersion
            ) =>
        {
            Ok(())
        }
        // Render the failure exactly as `lf` would on the CLI — usage line, the
        // specific problem, and any "did you mean" suggestion — so the door's
        // rejection reads identically to running the command yourself.
        Err(err) => Err(err.render().to_string()),
    }
}

/// Exec `lf <argv>` in `cwd` with `env` overlaid, wait, and capture. Assumes
/// the argv already passed [`validate_lf_argv`]. No shell: argv goes straight
/// to the binary.
pub(crate) async fn exec_lf(
    argv: &[String],
    cwd: Option<&str>,
    env: &[(String, String)],
) -> Result<LfExecResult, String> {
    let mut command = tokio::process::Command::new(resolve_lf_binary());
    command.args(argv).stdin(std::process::Stdio::null());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command
        .output()
        .await
        .map_err(|err| format!("failed to spawn lf: {err}"))?;
    Ok(LfExecResult {
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::validate_lf_argv;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn valid_mechanical_verb_parses() {
        assert!(validate_lf_argv(&argv(&["next", "--create-pr"])).is_ok());
        assert!(validate_lf_argv(&argv(&["pr", "land", "--strict"])).is_ok());
    }

    #[test]
    fn unknown_flag_is_rejected_before_exec() {
        let err = validate_lf_argv(&argv(&["next", "--nonesuch"]))
            .expect_err("unknown flag must not parse");
        assert!(err.contains("--nonesuch") || err.to_lowercase().contains("unexpected"));
    }

    #[test]
    fn empty_argv_is_rejected() {
        // Bare `lf` (no subcommand) has nothing to run.
        assert!(validate_lf_argv(&[]).is_err());
    }

    #[test]
    fn help_and_version_pass_through() {
        // `--help`/`--version` aren't failures — they must validate so the door
        // execs `lf` and the caller gets the real output, just like the CLI.
        assert!(validate_lf_argv(&argv(&["--help"])).is_ok());
        assert!(validate_lf_argv(&argv(&["--version"])).is_ok());
        assert!(validate_lf_argv(&argv(&["pr", "--help"])).is_ok());
    }

    #[test]
    fn error_message_reads_like_the_cli() {
        // A real parse error (unknown flag) should surface clap's full
        // rendering — the problem, a usage line, and the try-help hint — not a
        // bare string. (Unknown *subcommands* aren't errors: `lf` accepts
        // external subcommands, same as the CLI, so they fail at exec time.)
        let err = validate_lf_argv(&argv(&["next", "--nonesuch"]))
            .expect_err("unknown flag must not parse");
        assert!(err.contains("--nonesuch"), "names the bad token: {err}");
        assert!(
            err.to_lowercase().contains("usage") || err.to_lowercase().contains("--help"),
            "carries usage/help guidance like the CLI: {err}"
        );
    }
}
