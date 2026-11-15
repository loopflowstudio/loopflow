//! `lf config get|set|unset|explain`: the CLI adapter for the `config.*`
//! operations Desktop's Settings pane reaches through the server.
use std::path::Path;

use crate::config::files::Destination;

/// Which file a setting is declared in; `lf --wave <name>` selects a Wave's.
#[derive(clap::Args, Debug, Clone, Copy)]
pub struct DestinationArgs {
    /// Every repository
    #[arg(long)]
    pub global: bool,
    /// Edit the committed repository or Wave configuration explicitly
    #[arg(long, id = "config_shared", conflicts_with = "global")]
    pub shared: bool,
}

impl DestinationArgs {
    pub fn destination(self, repo: &Path, wave: Option<&str>) -> anyhow::Result<Destination> {
        Destination::from_cli(repo, wave, self.global, self.shared)
    }
}

/// `lf config set` with a value, `lf config unset` without one.
pub fn declare(
    destination: &Destination,
    key: &str,
    value: Option<&str>,
    json: bool,
) -> anyhow::Result<()> {
    let saved = match value {
        Some(value) => crate::engine::config::set(destination, key, value)?,
        None => crate::engine::config::unset(destination, key)?,
    };
    // The comma spelling still saves until LOO-875 removes it.
    if let Some(warning) = value
        .filter(|_| key.rsplit('.').next() == Some("default_agent"))
        .and_then(crate::config::settings::old_default_agent_warning)
    {
        eprintln!("warning: {}: {warning}", saved.path.display());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&saved)?);
    } else {
        println!(
            "{}: {key} {}; {:?}",
            saved.path.display(),
            if value.is_some() { "saved" } else { "unset" },
            saved.lifecycle
        );
    }
    Ok(())
}

pub fn get(destination: &Destination, key: &str, json: bool) -> anyhow::Result<()> {
    let declared = crate::engine::config::get(destination, key)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&declared)?);
    } else if declared.value.is_null() {
        println!("{}: no {key}", declared.path.display());
    } else {
        print!("{}", serde_yaml_ng::to_string(&declared.value)?);
    }
    Ok(())
}

/// The explanation is a source table; text and JSON print the same value.
pub fn explain(destination: &Destination, key: &str) -> anyhow::Result<()> {
    let explained = crate::engine::config::explain(destination, key)?;
    println!("{}", serde_json::to_string_pretty(&explained)?);
    Ok(())
}
