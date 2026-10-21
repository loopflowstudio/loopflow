//! Clipboard text for `lf -c`.
//!
//! Sources are tried in order until one yields text. On Linux the display
//! clipboards come first; the tmux paste buffer follows, because over SSH it
//! is where a person's copy lands and no display clipboard is reachable.

use std::process::Command;

/// One command that prints clipboard text on stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Source {
    program: String,
    args: Vec<String>,
}

impl Source {
    fn new(program: &str, args: &[&str]) -> Self {
        Self {
            program: program.to_string(),
            args: args.iter().map(|arg| arg.to_string()).collect(),
        }
    }
}

/// No source produced clipboard text; each attempt says why.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no clipboard text ({})", .attempts.join("; "))]
pub struct ClipboardUnavailable {
    pub attempts: Vec<String>,
}

/// Read text from the first clipboard source that has some.
///
/// # Errors
///
/// Returns every attempted source and why it produced no text.
pub fn read() -> Result<String, ClipboardUnavailable> {
    read_from(&sources())
}

fn sources() -> Vec<Source> {
    if cfg!(target_os = "macos") {
        return vec![Source::new("pbpaste", &[])];
    }
    let mut sources = vec![
        Source::new("xclip", &["-selection", "clipboard", "-o"]),
        Source::new("xsel", &["--clipboard", "--output"]),
        Source::new("wl-paste", &["--no-newline"]),
    ];
    if std::env::var_os("TMUX").is_some() {
        sources.push(Source::new("tmux", &["save-buffer", "-"]));
    }
    sources
}

fn read_from(sources: &[Source]) -> Result<String, ClipboardUnavailable> {
    let mut attempts = Vec::new();
    for source in sources {
        let name = &source.program;
        match Command::new(name).args(&source.args).output() {
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                attempts.push(format!("{name}: not installed"));
            }
            Err(err) => attempts.push(format!("{name}: {err}")),
            Ok(output) if !output.status.success() => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let reason = stderr.lines().next().unwrap_or("").trim();
                attempts.push(if reason.is_empty() {
                    format!("{name}: {}", output.status)
                } else {
                    format!("{name}: {reason}")
                });
            }
            Ok(output) => {
                let text = String::from_utf8_lossy(&output.stdout).into_owned();
                if !text.trim().is_empty() {
                    return Ok(text);
                }
                attempts.push(format!("{name}: empty"));
            }
        }
    }
    Err(ClipboardUnavailable { attempts })
}

#[cfg(all(test, unix))]
mod tests {
    use super::{read_from, Source};
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    fn stub(dir: &Path, name: &str, body: &str) -> Source {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        Source::new(path.to_str().unwrap(), &[])
    }

    #[test]
    fn failing_display_tool_falls_through_to_tmux_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let sources = [
            stub(
                dir.path(),
                "xclip",
                "echo 'Error: Can'\"'\"'t open display: (null)' >&2; exit 1",
            ),
            Source::new("/nonexistent/xsel", &[]),
            stub(dir.path(), "wl-paste", "exit 0"),
            stub(
                dir.path(),
                "tmux",
                "printf 'copied 漢字 ✓\\n\\tline two\\n'",
            ),
        ];

        assert_eq!(read_from(&sources).unwrap(), "copied 漢字 ✓\n\tline two\n");
    }

    #[test]
    fn no_text_names_every_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let sources = [
            stub(
                dir.path(),
                "xclip",
                "echo 'Error: Can'\"'\"'t open display: (null)' >&2; exit 1",
            ),
            Source::new("/nonexistent/xsel", &[]),
            stub(dir.path(), "tmux", "printf '  \\n'"),
        ];

        let attempts = read_from(&sources).unwrap_err().attempts;

        assert_eq!(attempts.len(), 3);
        assert!(attempts[0].ends_with("xclip: Error: Can't open display: (null)"));
        assert_eq!(attempts[1], "/nonexistent/xsel: not installed");
        assert!(attempts[2].ends_with("tmux: empty"));
    }
}
