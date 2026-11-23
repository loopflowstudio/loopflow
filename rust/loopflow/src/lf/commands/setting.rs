//! `lf config get|set|unset|explain`: the CLI adapter for the `config.*`
//! operations Desktop's Settings pane reaches through the server.
use loopflow_execution::engine::config::ScopeTarget;
use std::path::Path;

use loopflow_execution::config::files::Destination;

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
        Some(value) => loopflow_execution::engine::config::set(destination, key, value)?,
        None => loopflow_execution::engine::config::unset(destination, key)?,
    };
    // The comma spelling still saves until LOO-875 removes it.
    if let Some(warning) = value
        .filter(|_| key.rsplit('.').next() == Some("default_agent"))
        .and_then(loopflow_execution::config::settings::old_default_agent_warning)
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
    let declared = loopflow_execution::engine::config::get(destination, key)?;
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
    let explained = loopflow_execution::engine::config::explain(destination, key)?;
    println!("{}", serde_json::to_string_pretty(&explained)?);
    Ok(())
}

pub fn scope(
    directory: Option<&Path>,
    wave: Option<&str>,
    target: ScopeTarget,
    subscribe: bool,
) -> anyhow::Result<()> {
    if subscribe {
        return subscribe_scope(
            directory.map(Path::to_owned),
            wave.map(str::to_owned),
            target,
            loopflow_execution::read_stream::Transport::Stdio,
        )
        .map(drop);
    }
    let scope = loopflow_execution::engine::config::scope(directory, wave, &target)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&loopflow_execution::read_stream::selected_answer(&scope)?)?
    );
    Ok(())
}

pub(super) fn subscribe_scope(
    directory: Option<std::path::PathBuf>,
    wave: Option<String>,
    target: ScopeTarget,
    transport: loopflow_execution::read_stream::Transport,
) -> anyhow::Result<Option<loopflow_execution::read_stream::Subscription>> {
    loopflow_execution::read_stream::start(
        loopflow_execution::read_stream::Watch::Files,
        (),
        move |_, ()| {
            let directory = directory.clone();
            let wave = wave.clone();
            let target = target.clone();
            async move {
                loopflow_execution::engine::config::scope(
                    directory.as_deref(),
                    wave.as_deref(),
                    &target,
                )
            }
        },
        transport,
    )
}
