//! Inspect and edit configuration from the CLI using the same operations as Desktop.
use std::path::{Path, PathBuf};

use loopflow_execution::config::files::Destination;
use loopflow_execution::engine::config::{scope, ScopeTarget};
use loopflow_execution::read_stream::{self, Subscription, Transport, Watch};

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

#[derive(clap::Args, Debug)]
pub struct ScopeArgs {
    /// Read only your global preferences, outside any checkout
    #[arg(long)]
    global: bool,
    /// Resolve the settings a launch of this Skill gets
    #[arg(long, conflicts_with = "graph")]
    skill: Option<String>,
    /// Resolve the settings this Graph's --step gets
    #[arg(long, requires = "step")]
    graph: Option<String>,
    /// The Graph step to resolve
    #[arg(long, requires = "graph")]
    step: Option<String>,
    /// Print JSON (the only format; required by --subscribe)
    #[arg(long)]
    json: bool,
    /// Print a reading whenever a settings file changes, until stdin closes
    #[arg(short = 's', long, requires = "json")]
    subscribe: bool,
    #[command(flatten)]
    selection: crate::lf::Selection,
}

impl ScopeArgs {
    pub fn run(
        &self,
        wave: Option<&str>,
        checkout: impl FnOnce() -> anyhow::Result<PathBuf>,
    ) -> anyhow::Result<()> {
        let directory = (!self.global).then(checkout).transpose()?;
        let target = ScopeTarget::from_options(
            self.skill.as_deref(),
            self.graph.as_deref(),
            self.step.as_deref(),
        )?;
        read_stream::select_output(
            (!self.selection.select.is_empty()).then(|| self.selection.select.clone()),
        );
        if self.subscribe {
            return subscribe_scope(directory, wave.map(str::to_owned), target, Transport::Stdio)
                .map(drop);
        }
        let reading = scope(directory.as_deref(), wave, &target)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&read_stream::selected_answer(&reading)?)?
        );
        Ok(())
    }
}

pub(super) fn subscribe_scope(
    directory: Option<PathBuf>,
    wave: Option<String>,
    target: ScopeTarget,
    transport: Transport,
) -> anyhow::Result<Option<Subscription>> {
    read_stream::start(
        Watch::Files,
        (),
        move |_, ()| std::future::ready(scope(directory.as_deref(), wave.as_deref(), &target)),
        transport,
    )
}
