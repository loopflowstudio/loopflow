//! Standalone transport for shared reads and commands; owns no execution.
pub mod client;
pub mod schema;
mod service;
pub mod wire;

use std::path::PathBuf;

pub fn socket_path() -> anyhow::Result<PathBuf> {
    // Keep the installed endpoint stable: relays from before the namespace
    // cutover must reconnect to this same listener after an update.
    Ok(crate::transport::private_directory()?.join("machine.sock"))
}

/// `PATH` with the directories a GUI-started process lacks: an OS supervisor
/// gives this service the same minimal `PATH` it gives the app, and everything
/// the server starts (`gh`, a provider) must resolve as it does in a terminal.
fn supervised_path(path: &str, home: &std::path::Path) -> String {
    let mut entries: Vec<String> = path
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned)
        .collect();
    for entry in crate::world::DESKTOP_PATH.iter().rev() {
        let entry = match entry.strip_prefix("~/") {
            Some(rest) => home.join(rest).to_string_lossy().into_owned(),
            None => (*entry).to_owned(),
        };
        if !entries.contains(&entry) {
            entries.insert(0, entry);
        }
    }
    entries.join(":")
}

/// Set by the launchd and systemd definitions. A server started from a
/// terminal keeps that terminal's `PATH`.
pub(crate) const SUPERVISED_ENV: &str = "LF_SUPERVISED";

pub fn run() -> anyhow::Result<()> {
    // Before the runtime's threads exist: nothing reads the environment yet.
    if std::env::var_os(SUPERVISED_ENV).is_some() {
        std::env::remove_var(SUPERVISED_ENV);
        std::env::set_var(
            "PATH",
            supervised_path(
                &std::env::var("PATH").unwrap_or_default(),
                &dirs::home_dir().unwrap_or_default(),
            ),
        );
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(service::run())
}

#[cfg(test)]
mod tests {
    use super::supervised_path;
    use std::path::Path;

    #[test]
    fn a_supervised_server_finds_what_a_terminal_finds() {
        let path = supervised_path("/usr/bin:/bin:/opt/homebrew/bin", Path::new("/Users/maya"));
        let entries: Vec<_> = path.split(':').collect();
        assert_eq!(entries[0], "/Users/maya/.local/bin");
        assert!(entries.contains(&"/Users/maya/.cargo/bin"));
        assert_eq!(
            entries
                .iter()
                .filter(|e| **e == "/opt/homebrew/bin")
                .count(),
            1
        );
        assert!(entries.ends_with(&["/usr/bin", "/bin", "/opt/homebrew/bin"]));
    }
}
