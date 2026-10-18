//! The `loopflow:` receiver installed with the CLI, so a clicked Work link
//! opens when Desktop is absent. It contains no Work policy: it hands only the
//! URL to `lf open`, dropping inherited host context. An OS receiver's launch
//! environment belongs to whichever host first launched it, not to the click.
//! Installed Desktop receives `loopflow:` itself; another app's claim is kept.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};

const SCHEME_TYPE: &str = "x-scheme-handler/loopflow";
const DESKTOP_ENTRY: &str = "loopflow-links.desktop";
const APPLET: &str = "Loopflow Links.app";
const APPLET_BUNDLE: &str = "com.loopflow.links";
const DESKTOP_BUNDLE: &str = "com.loopflow.mac";

/// Who receives `loopflow:` links after installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Receiver {
    /// The CLI's receiver at this path.
    Installed(PathBuf),
    /// Loopflow Desktop is installed and receives links itself.
    Desktop,
    /// Another application holds the scheme; it was left in place.
    Other(String),
}

/// The receiver's argv before the URL: `lf open` without terminal-host context.
pub fn receiver_argv(lf: &Path) -> Vec<String> {
    let mut argv = vec!["/usr/bin/env".to_owned()];
    for name in crate::lf::commands::open::HOST_CONTEXT {
        argv.extend(["-u".to_owned(), (*name).to_owned()]);
    }
    argv.extend([lf.display().to_string(), "open".into(), "--".into()]);
    argv
}

/// Install or refresh the receiver for this OS account. `desktop` is a Desktop
/// bundle this installation just placed, before Launch Services may index it.
pub fn install(home: &Path, lf: &Path, desktop: Option<&Path>) -> Result<Receiver> {
    if cfg!(target_os = "macos") {
        install_macos(home, lf, desktop.is_some_and(Path::exists))
    } else {
        let data = env_dir("XDG_DATA_HOME").unwrap_or_else(|| home.join(".local/share"));
        let config = env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config"));
        install_xdg(&data, &config, lf)
    }
}

fn env_dir(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

// ---------------------------------------------------------------------------
// Linux and other XDG desktops
// ---------------------------------------------------------------------------

/// Write the desktop entry and select it in `mimeapps.list` unless another
/// handler was chosen for `loopflow:`.
pub fn install_xdg(data: &Path, config: &Path, lf: &Path) -> Result<Receiver> {
    let applications = data.join("applications");
    fs::create_dir_all(&applications)
        .with_context(|| format!("create {}", applications.display()))?;
    let entry = applications.join(DESKTOP_ENTRY);
    let exec = receiver_argv(lf)
        .iter()
        .map(|arg| desktop_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");
    write_atomic(
        &entry,
        &format!(
            "[Desktop Entry]\nType=Application\nName=Loopflow Links\n\
             Comment=Open Loopflow Work links\nExec={exec} %u\nMimeType={SCHEME_TYPE};\n\
             NoDisplay=true\nTerminal=false\n"
        ),
    )?;
    let list = config.join("mimeapps.list");
    let current = match fs::read_to_string(&list) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error).with_context(|| format!("read {}", list.display())),
    };
    match default_handler(&current) {
        Some(handler) if handler != DESKTOP_ENTRY => return Ok(Receiver::Other(handler)),
        Some(_) => {}
        None => {
            fs::create_dir_all(config).with_context(|| format!("create {}", config.display()))?;
            write_atomic(&list, &with_default_handler(&current))?;
        }
    }
    Ok(Receiver::Installed(entry))
}

/// Desktop Entry `Exec` quoting: every argument double-quoted, with `"`, `` ` ``,
/// `$` and `\` escaped, and `%` doubled so no field code expands.
fn desktop_quote(arg: &str) -> String {
    let mut quoted = String::from("\"");
    for character in arg.chars() {
        match character {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(character);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(character),
        }
    }
    quoted.push('"');
    quoted
}

fn default_handler(list: &str) -> Option<String> {
    let mut in_defaults = false;
    for line in list.lines().map(str::trim) {
        if line.starts_with('[') {
            in_defaults = line == "[Default Applications]";
        } else if in_defaults {
            if let Some(value) = line
                .strip_prefix(SCHEME_TYPE)
                .and_then(|rest| rest.trim_start().strip_prefix('='))
            {
                return value
                    .split(';')
                    .map(str::trim)
                    .find(|handler| !handler.is_empty())
                    .map(str::to_owned);
            }
        }
    }
    None
}

fn with_default_handler(list: &str) -> String {
    let line = format!("{SCHEME_TYPE}={DESKTOP_ENTRY}");
    let mut lines: Vec<String> = list.lines().map(str::to_owned).collect();
    match lines
        .iter()
        .position(|line| line.trim() == "[Default Applications]")
    {
        Some(section) => lines.insert(section + 1, line),
        None => {
            if lines.last().is_some_and(|last| !last.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.extend(["[Default Applications]".to_owned(), line]);
        }
    }
    lines.join("\n") + "\n"
}

fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let parent = path.parent().expect("receiver files have a parent");
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut temporary, contents.as_bytes())?;
    temporary
        .persist(path)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// macOS
// ---------------------------------------------------------------------------

fn install_macos(home: &Path, lf: &Path, placed_desktop: bool) -> Result<Receiver> {
    let applet = home
        .join("Library/Application Support/Loopflow")
        .join(APPLET);
    if placed_desktop || application_for_bundle(DESKTOP_BUNDLE).is_some() {
        // Desktop owns the scheme and resolves through `lf open --link` itself.
        if applet.exists() {
            fs::remove_dir_all(&applet).with_context(|| format!("remove {}", applet.display()))?;
        }
        return Ok(Receiver::Desktop);
    }
    if let Some(handler) = scheme_handler().filter(|handler| *handler != applet) {
        return Ok(Receiver::Other(handler.display().to_string()));
    }
    write_applet(&applet, lf)?;
    let status = Command::new(
        "/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister",
    )
    .arg("-f")
    .arg(&applet)
    .status()
    .context("register the link receiver with Launch Services")?;
    anyhow::ensure!(
        status.success(),
        "Launch Services registration exited with {status}"
    );
    Ok(Receiver::Installed(applet))
}

/// Compile an AppleScript applet whose `open location` handler runs the
/// receiver argv with the clicked URL. Failures surface with `lf open`'s
/// own explanation and fallback command.
pub fn write_applet(applet: &Path, lf: &Path) -> Result<()> {
    let parent = applet.parent().expect("applet has a parent");
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let command = shlex::try_join(receiver_argv(lf).iter().map(String::as_str))?;
    let staged = tempfile::Builder::new()
        .prefix(".links")
        .tempdir_in(parent)?;
    let built = staged.path().join(APPLET);
    let status = Command::new("osacompile")
        .arg("-o")
        .arg(&built)
        .args([
            "-e",
            "on open location workURL",
            "-e",
            &format!(
                "do shell script {} & \" \" & quoted form of workURL & \" 2>&1\"",
                applescript_string(&command)
            ),
            "-e",
            "end open location",
        ])
        .status()
        .context("compile the link receiver with osacompile")?;
    anyhow::ensure!(status.success(), "osacompile exited with {status}");
    let info = built.join("Contents/Info.plist");
    for (key, kind, value) in [
        ("CFBundleIdentifier", "string", APPLET_BUNDLE),
        ("CFBundleName", "string", "Loopflow Links"),
        ("LSUIElement", "bool", "true"),
        (
            "CFBundleURLTypes",
            "json",
            r#"[{"CFBundleURLName":"studio.loopflow.links","CFBundleURLSchemes":["loopflow"]}]"#,
        ),
    ] {
        let status = Command::new("plutil")
            .args(["-replace", key, &format!("-{kind}"), value])
            .arg(&info)
            .status()
            .context("edit the link receiver's Info.plist")?;
        anyhow::ensure!(status.success(), "plutil could not set {key}");
    }
    if applet.exists() {
        fs::remove_dir_all(applet).with_context(|| format!("replace {}", applet.display()))?;
    }
    fs::rename(&built, applet).with_context(|| format!("install {}", applet.display()))?;
    Ok(())
}

fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Launch Services lookups through JavaScript for Automation; no permission prompt.
fn launch_services(expression: &str) -> Option<PathBuf> {
    let output = Command::new("osascript")
        .args([
            "-l",
            "JavaScript",
            "-e",
            &format!("ObjC.import('AppKit'); var u = {expression}; u.isNil() ? '' : u.path.js"),
        ])
        .output()
        .ok()?;
    let path = String::from_utf8(output.stdout).ok()?;
    let path = path.trim();
    (output.status.success() && !path.is_empty()).then(|| PathBuf::from(path))
}

fn application_for_bundle(bundle: &str) -> Option<PathBuf> {
    launch_services(&format!(
        "$.NSWorkspace.sharedWorkspace.URLForApplicationWithBundleIdentifier('{bundle}')"
    ))
}

fn scheme_handler() -> Option<PathBuf> {
    launch_services(
        "$.NSWorkspace.sharedWorkspace.URLForApplicationToOpenURL($.NSURL.URLWithString('loopflow://open'))",
    )
}

#[cfg(test)]
mod tests {
    use super::{install_xdg, receiver_argv, write_applet, Receiver};
    use std::path::Path;

    #[test]
    fn receiver_passes_only_the_url_to_lf_open() {
        let argv = receiver_argv(Path::new("/opt/lf bin/lf"));
        assert_eq!(&argv[argv.len() - 3..], ["/opt/lf bin/lf", "open", "--"]);
        for name in crate::lf::commands::open::HOST_CONTEXT {
            assert!(argv.windows(2).any(|pair| pair == ["-u", *name]));
        }
    }

    #[test]
    fn xdg_receiver_claims_an_unclaimed_scheme_and_refreshes_in_place() {
        let root = tempfile::tempdir().unwrap();
        let (data, config) = (root.path().join("data"), root.path().join("config"));
        std::fs::create_dir_all(&config).unwrap();
        std::fs::write(
            config.join("mimeapps.list"),
            "[Added Associations]\ntext/plain=editor.desktop;\n",
        )
        .unwrap();
        let lf = Path::new("/home/jack/.local/bin/lf");
        let first = install_xdg(&data, &config, lf).unwrap();
        let again = install_xdg(&data, &config, lf).unwrap();
        assert_eq!(first, again);
        let Receiver::Installed(entry) = first else {
            panic!("expected the CLI receiver")
        };
        let entry = std::fs::read_to_string(entry).unwrap();
        assert!(entry.contains("MimeType=x-scheme-handler/loopflow;"));
        assert!(entry.contains("\"/home/jack/.local/bin/lf\" \"open\" \"--\" %u"));
        let list = std::fs::read_to_string(config.join("mimeapps.list")).unwrap();
        assert!(list.contains("text/plain=editor.desktop;"));
        assert_eq!(
            list.matches("x-scheme-handler/loopflow=loopflow-links.desktop")
                .count(),
            1
        );
    }

    #[test]
    fn xdg_receiver_keeps_another_chosen_handler() {
        let root = tempfile::tempdir().unwrap();
        let (data, config) = (root.path().join("data"), root.path().join("config"));
        std::fs::create_dir_all(&config).unwrap();
        let chosen = "[Default Applications]\nx-scheme-handler/loopflow=custom.desktop;\n";
        std::fs::write(config.join("mimeapps.list"), chosen).unwrap();
        let receiver = install_xdg(&data, &config, Path::new("/usr/bin/lf")).unwrap();
        assert_eq!(receiver, Receiver::Other("custom.desktop".into()));
        assert_eq!(
            std::fs::read_to_string(config.join("mimeapps.list")).unwrap(),
            chosen
        );
    }

    #[test]
    fn desktop_entry_quoting_never_expands_field_codes() {
        let root = tempfile::tempdir().unwrap();
        let (data, config) = (root.path().join("data"), root.path().join("config"));
        install_xdg(&data, &config, Path::new("/tmp/100%$\"lf")).unwrap();
        let entry =
            std::fs::read_to_string(data.join("applications/loopflow-links.desktop")).unwrap();
        assert!(entry.contains(r#""/tmp/100%%\$\"lf""#));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn applet_declares_the_scheme_and_quotes_the_url() {
        let root = tempfile::tempdir().unwrap();
        let applet = root.path().join("Loopflow Links.app");
        write_applet(&applet, Path::new("/Users/jack/.local/bin/lf")).unwrap();
        write_applet(&applet, Path::new("/Users/jack/.local/bin/lf")).unwrap();
        let info = std::process::Command::new("plutil")
            .args(["-convert", "json", "-o", "-"])
            .arg(applet.join("Contents/Info.plist"))
            .output()
            .unwrap();
        let info: serde_json::Value = serde_json::from_slice(&info.stdout).unwrap();
        assert_eq!(info["CFBundleIdentifier"], "com.loopflow.links");
        assert_eq!(
            info["CFBundleURLTypes"][0]["CFBundleURLSchemes"][0],
            "loopflow"
        );
        let script = std::process::Command::new("osadecompile")
            .arg(applet.join("Contents/Resources/Scripts/main.scpt"))
            .output()
            .unwrap();
        let script = String::from_utf8(script.stdout).unwrap();
        assert!(script.contains("quoted form of workURL"));
        assert!(script.contains("/Users/jack/.local/bin/lf open --"));
    }
}
