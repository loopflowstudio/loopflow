//! Commands for native workspace panes. Hosts launch these in the Task checkout.
use std::path::Path;

use anyhow::{Context, Result};

pub(super) fn shell() -> Vec<String> {
    vec![
        std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()),
        "-l".into(),
    ]
}

pub(super) fn files(checkout: &Path, path: Option<&str>) -> Result<Vec<String>> {
    let path = Path::new(path.unwrap_or("."));
    anyhow::ensure!(
        !path.is_absolute()
            && !path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir)),
        "Files path must stay inside the recorded checkout"
    );
    let full = checkout
        .join(path)
        .canonicalize()
        .context("Files target is unavailable")?;
    anyhow::ensure!(
        full.starts_with(checkout.canonicalize()?),
        "Files target is outside the recorded checkout"
    );
    let editor = if full.is_dir() {
        std::env::var("LF_FILE_BROWSER").ok()
    } else {
        None
    }
    .or_else(|| std::env::var("VISUAL").ok())
    .or_else(|| std::env::var("EDITOR").ok())
    .filter(|value| !value.trim().is_empty())
    .unwrap_or_else(|| "vi".into());
    Ok(editor_argv(&editor, &full))
}

fn editor_argv(editor: &str, path: &Path) -> Vec<String> {
    // Editor settings are user-authored shell commands. Paths stay positional.
    vec![
        "/bin/sh".into(),
        "-c".into(),
        format!("{editor} \"$@\""),
        "lf-files".into(),
        path.to_string_lossy().into_owned(),
    ]
}

#[cfg(test)]
mod tests {
    use super::editor_argv;
    use std::path::Path;
    use std::process::Command;

    fn output(editor: &str, path: &Path) -> std::process::Output {
        let argv = editor_argv(editor, path);
        Command::new(&argv[0]).args(&argv[1..]).output().unwrap()
    }

    #[test]
    fn editor_setting_can_include_environment_assignments() {
        let output = output(
            r#"EDITOR_TEST_FLAG=present sh -c 'printf "%s\n" "$EDITOR_TEST_FLAG" "$1"' editor"#,
            Path::new("/tmp/selected file"),
        );
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "present\n/tmp/selected file\n"
        );
    }

    #[test]
    fn editor_receives_literal_paths_after_its_options() {
        let output = output(
            "printf '%s\\n' --",
            Path::new("/tmp/space ' $(touch never);x"),
        );
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "--\n/tmp/space ' $(touch never);x\n"
        );
    }
}
