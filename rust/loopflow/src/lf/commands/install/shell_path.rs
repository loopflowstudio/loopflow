//! Keep the installed `lf` directory on the default Mac zsh PATH.
//!
//! An installer cannot change its parent shell, and a fresh Mac does not put
//! `~/.local/bin` on PATH. Installation therefore owns one marked block in
//! `${ZDOTDIR:-$HOME}/.zshrc` that conditionally prepends the install
//! directory. Reinstalling replaces that block; everything else in the file is
//! preserved byte for byte. Other shells get an explicit instruction.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};

use crate::os_process::shell_escape;

const BEGIN: &str = "# >>> Loopflow PATH >>>";
const END: &str = "# <<< Loopflow PATH <<<";

/// The interactive zsh startup file, honoring `ZDOTDIR` the way zsh does.
fn zsh_startup_file() -> Result<PathBuf> {
    let directory = match std::env::var_os("ZDOTDIR").filter(|value| !value.is_empty()) {
        Some(directory) => PathBuf::from(directory),
        None => dirs::home_dir().context("cannot determine the home directory for zsh")?,
    };
    Ok(directory.join(".zshrc"))
}

fn utf8(directory: &Path) -> Result<&str> {
    directory.to_str().ok_or_else(|| {
        anyhow!(
            "install directory {} is not valid UTF-8",
            directory.display()
        )
    })
}

/// The command that puts `directory` on PATH for the current shell only.
pub fn export_command(directory: &Path) -> Result<String> {
    Ok(format!(
        "export PATH={}:\"$PATH\"",
        shell_escape(utf8(directory)?)
    ))
}

fn render_block(directory: &Path) -> Result<String> {
    let quoted = shell_escape(utf8(directory)?);
    Ok(format!(
        "{BEGIN}\n\
         # Added by the Loopflow installer; reinstalling replaces this block.\n\
         case \":$PATH:\" in\n  \
         *:{quoted}:*) ;;\n  \
         *) export PATH={quoted}\"${{PATH:+:$PATH}}\" ;;\n\
         esac\n\
         {END}\n"
    ))
}

/// Replace the owned block in place, or append it after existing content.
fn with_block(existing: &str, block: &str) -> Result<String> {
    let begin = existing
        .match_indices(BEGIN)
        .map(|(index, _)| index)
        .find(|index| *index == 0 || existing.as_bytes()[index - 1] == b'\n');
    if let Some(begin) = begin {
        let Some(end) = existing[begin..].find(END).map(|offset| begin + offset) else {
            return Err(anyhow!(
                "the Loopflow PATH block has no closing `{END}` line"
            ));
        };
        let after = existing[end..]
            .find('\n')
            .map(|offset| end + offset + 1)
            .unwrap_or(existing.len());
        return Ok(format!(
            "{}{block}{}",
            &existing[..begin],
            &existing[after..]
        ));
    }
    let separator = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    Ok(format!("{existing}{separator}{block}"))
}

fn write_preserving(path: &Path, contents: &str) -> Result<()> {
    // Dotfile managers symlink .zshrc; update the file they manage.
    let path = match fs::canonicalize(path) {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => path.to_path_buf(),
        Err(error) => return Err(error).with_context(|| format!("resolve {}", path.display())),
    };
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("{} has no parent directory", path.display()))?;
    let mode = match fs::metadata(&path) {
        Ok(metadata) => metadata.permissions().mode(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0o644,
        Err(error) => return Err(error).with_context(|| format!("inspect {}", path.display())),
    };
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("write beside {}", path.display()))?;
    temporary.write_all(contents.as_bytes())?;
    temporary.as_file().sync_all()?;
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(mode))?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)
        .with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

/// Ensure new interactive zsh sessions find `directory`; returns the file.
pub fn persist(directory: &Path) -> Result<PathBuf> {
    let file = zsh_startup_file()?;
    persist_in(&file, directory)?;
    Ok(file)
}

fn persist_in(file: &Path, directory: &Path) -> Result<()> {
    let block = render_block(directory)?;
    let failure = |error: anyhow::Error| {
        let repair = export_command(directory).unwrap_or_default();
        error.context(format!(
            "Loopflow is installed, but {} could not be updated. Add this line to it, then open a new terminal: {repair}",
            file.display()
        ))
    };
    let existing = match fs::read(file) {
        Ok(bytes) => String::from_utf8(bytes)
            .map_err(|_| anyhow!("{} is not valid UTF-8", file.display()))
            .map_err(failure)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(failure(anyhow::Error::from(error))),
    };
    let updated = with_block(&existing, &block).map_err(failure)?;
    if updated != existing {
        write_preserving(file, &updated).map_err(failure)?;
    }
    Ok(())
}

/// `lf install shell-path`: the shell installer's entry to the same operation.
pub fn configure(directory: Option<&Path>) -> Result<()> {
    let directory = match directory {
        Some(directory) => std::path::absolute(directory)?,
        None => super::published::install_dir()?,
    };
    let directory = directory.as_path();
    if !cfg!(target_os = "macos") {
        println!(
            "Add {} to PATH in your shell's startup file: {}",
            directory.display(),
            export_command(directory)?
        );
        return Ok(());
    }
    let file = persist(directory)?;
    println!("New zsh terminals find lf: {}", file.display());
    let other_shell = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|shell| shell.file_name().is_some_and(|name| name != "zsh"));
    if let Some(shell) = other_shell {
        println!(
            "Your shell is {}; add this to its startup file: {}",
            shell.display(),
            export_command(directory)?
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{export_command, persist_in, render_block, with_block, BEGIN};
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::process::Command;

    fn zsh_finds(rc: &str, directory: &Path) -> bool {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join(".zshrc"), rc).unwrap();
        let output = Command::new("/bin/zsh")
            .args(["-ic", "print -r -- \"$PATH\""])
            .env_clear()
            .env("HOME", home.path())
            .env("ZDOTDIR", home.path())
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .trim()
            .split(':')
            .any(|entry| Path::new(entry) == directory)
    }

    #[test]
    fn block_is_appended_once_and_preserves_unterminated_content() {
        let block = render_block(Path::new("/Users/kim/.local/bin")).unwrap();
        let first = with_block("alias ll='ls -l'", &block).unwrap();
        assert!(first.starts_with("alias ll='ls -l'\n# >>> Loopflow PATH >>>\n"));
        assert_eq!(with_block(&first, &block).unwrap(), first);
        let moved = render_block(Path::new("/opt/lf bin")).unwrap();
        let replaced = with_block(&format!("{first}export EDITOR=vi\n"), &moved).unwrap();
        assert_eq!(replaced.matches(BEGIN).count(), 1);
        assert!(replaced.ends_with("# <<< Loopflow PATH <<<\nexport EDITOR=vi\n"));
        assert!(replaced.contains("'/opt/lf bin'"));
        assert!(with_block(&format!("{BEGIN}\nunfinished\n"), &block).is_err());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn fresh_zsh_resolves_paths_with_spaces_and_metacharacters() {
        for directory in ["/tmp/plain/bin", "/tmp/Kim's $HOME `x` dir/bin"] {
            let directory = Path::new(directory);
            let rc = with_block("", &render_block(directory).unwrap()).unwrap();
            assert!(zsh_finds(&rc, directory), "{rc}");
            assert!(!zsh_finds("", directory));
        }
        assert_eq!(
            export_command(Path::new("/a'b")).unwrap(),
            r#"export PATH='/a'\''b':"$PATH""#
        );
    }

    #[test]
    fn dotfile_symlink_is_updated_and_unwritable_profile_names_the_repair() {
        let home = tempfile::tempdir().unwrap();
        let managed = home.path().join("dotfiles/zshrc");
        fs::create_dir(managed.parent().unwrap()).unwrap();
        fs::write(&managed, "setopt autocd").unwrap();
        let link = home.path().join(".zshrc");
        std::os::unix::fs::symlink(&managed, &link).unwrap();
        persist_in(&link, Path::new("/tmp/lf/bin")).unwrap();
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(fs::read_to_string(&managed)
            .unwrap()
            .starts_with("setopt autocd\n# >>> Loopflow PATH >>>"));

        let locked = home.path().join("locked");
        fs::create_dir(&locked).unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o555)).unwrap();
        let error = persist_in(&locked.join(".zshrc"), Path::new("/tmp/lf/bin")).unwrap_err();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        let message = format!("{error:#}");
        assert!(message.contains("Loopflow is installed, but"), "{message}");
        assert!(
            message.contains("export PATH='/tmp/lf/bin':\"$PATH\""),
            "{message}"
        );
    }
}
