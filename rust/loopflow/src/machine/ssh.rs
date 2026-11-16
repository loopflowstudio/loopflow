//! Bounded request/reply transport to an identified Machine.
use anyhow::Context;
use std::process::Stdio;

pub const EXPECTED_MACHINE_ID_ENV: &str = "LF_EXPECTED_MACHINE_ID";
// Non-interactive SSH skips shell profiles, including the npm prefix.
pub(crate) const REMOTE_PATH: &str = "export PATH=\"$HOME/.cargo/bin:$HOME/.local/bin:/opt/homebrew/bin:/usr/local/bin:$PATH\"; if command -v npm >/dev/null 2>&1; then PATH=\"$PATH:$(npm config get prefix 2>/dev/null)/bin\"; fi";

pub(crate) fn build_preamble(cmd: &[String], extra_env: &[(&str, &str)]) -> String {
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
pub(crate) fn sh_quote(value: &str) -> String {
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
pub(crate) async fn request(
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
