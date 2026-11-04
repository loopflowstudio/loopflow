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
    let file = crate::config::files::set(repo, wave, global, shared, key, value)?;
    let preference = crate::config::settings::preference_path(key)?;
    let lifecycle = crate::config::catalog::definition(preference[0])
        .expect("validated preference")
        .lifecycle;
    println!(
        "{}: {key} {}; {lifecycle:?}",
        file.display(),
        if value.is_some() { "saved" } else { "unset" }
    );
    Ok(())
}
