use anyhow::{anyhow, Result};
use std::path::PathBuf;

pub fn find_repo_root() -> Result<PathBuf> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()?;

    if !output.status.success() {
        return Ok(std::env::current_dir()?);
    }

    Ok(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
}

pub fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::io::Write;

    let mut child = std::process::Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| anyhow!("pbcopy stdin unavailable"))?;
    stdin.write_all(text.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        return Err(anyhow!("pbcopy failed"));
    }
    Ok(())
}

pub fn open_web_client(backend: &str) -> Result<()> {
    let url = match backend {
        "claude" => "https://claude.ai/new",
        "codex" => "https://chatgpt.com",
        "gemini" => "https://aistudio.google.com/prompts/new_chat",
        _ => "https://claude.ai/new",
    };
    std::process::Command::new("open").arg(url).spawn()?;
    Ok(())
}
