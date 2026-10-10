use std::io::Write;
use std::process::{Command, Stdio};

/// Read text from the system clipboard.
///
/// Returns `None` if the clipboard is empty or no clipboard tool is available.
pub fn read() -> Option<String> {
    let output = if cfg!(target_os = "macos") {
        Command::new("pbpaste").output().ok()?
    } else {
        try_command("xclip", &["-selection", "clipboard", "-o"])
            .or_else(|| try_command("xsel", &["--clipboard", "--output"]))
            .or_else(|| try_command("wl-paste", &[]))?
    };

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    None
}

/// Write text to the system clipboard.
pub fn write(text: &str) -> Result<(), std::io::Error> {
    if cfg!(target_os = "macos") {
        write_via("pbcopy", &[], text)
    } else {
        write_via("xclip", &["-selection", "clipboard"], text)
            .or_else(|_| write_via("xsel", &["--clipboard", "--input"], text))
            .or_else(|_| write_via("wl-copy", &[], text))
    }
}

fn try_command(cmd: &str, args: &[&str]) -> Option<std::process::Output> {
    Command::new(cmd).args(args).output().ok()
}

fn write_via(cmd: &str, args: &[&str], text: &str) -> Result<(), std::io::Error> {
    let mut child = Command::new(cmd).args(args).stdin(Stdio::piped()).spawn()?;

    if let Some(ref mut stdin) = child.stdin {
        stdin.write_all(text.as_bytes())?;
    }

    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::other(format!("{cmd} exited with {status}")));
    }
    Ok(())
}
