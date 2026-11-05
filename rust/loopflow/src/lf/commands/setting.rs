//! `lf set` and `lf unset` edit the same typed declarations as files and Desktop.
use std::path::Path;

pub fn run(
    repo: &Path,
    wave: Option<&str>,
    key: &str,
    value: Option<&str>,
    global: bool,
    shared: bool,
) -> anyhow::Result<()> {
    let destination = crate::config::files::Destination::from_cli(repo, wave, global, shared)?;
    let saved = crate::config::api::declaration(&destination, key, value, value.is_none())?;
    println!(
        "{}: {key} {}; {:?}",
        saved.path.display(),
        if value.is_some() { "saved" } else { "unset" },
        saved.lifecycle
    );
    Ok(())
}
