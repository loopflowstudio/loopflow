//! Rename or relocate an authored Wave and its provider mapping.
use crate::lf::WaveCommand;
use std::path::Path;

pub fn rename(repo: &Path, command: &WaveCommand) -> anyhow::Result<()> {
    let WaveCommand::Rename {
        wave,
        new_name,
        repo: target,
        title,
        json,
    } = command
    else {
        unreachable!("Wave rename dispatcher");
    };
    let renamed = tokio::runtime::Runtime::new()?.block_on(crate::ops::wave_lifecycle::rename(
        repo,
        wave,
        new_name.as_deref(),
        target.as_deref(),
        title.as_deref(),
    ))?;
    if *json {
        println!("{}", serde_json::to_string_pretty(&renamed)?);
    } else {
        println!("{}", crate::ops::wave_lifecycle::render_renamed(&renamed));
    }
    Ok(())
}
