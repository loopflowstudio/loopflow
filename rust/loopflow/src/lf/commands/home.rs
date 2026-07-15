//! Route a top-level `lf` command to its Wave's execution home.
//!
//! A repo/PR/release/PM command belongs where the Wave's work lives. When the
//! resolved Wave's authored home is a remote SSH target, the command is
//! forwarded there over the existing `lf ssh` credential-forwarding transport
//! instead of running locally; a `local` (or absent) home changes nothing.
//!
//! The home is read from the resolved Wave's identity — the pinned/inherited
//! `LF_WAVE_HOME`, else that Wave's `GOAL.md` — never a string parsed out of a
//! branch or path. Read-only and lifecycle commands (`status`, `wave`, the
//! runners, `ssh` itself) stay local so an operator can always see and steer
//! both a local and a remote Wave from this machine.

use std::path::Path;

use anyhow::anyhow;

use crate::engine::wave_config::{default_local_home, read_wave_home};
use crate::engine::wave_context::resolve_run_wave_name;
use crate::engine::wave_home::{
    HomeActionDto, HomeRuntimeDto, HomeState, WaveHome, HOME_ROUTED_ENV, WAVE_HOME_ENV,
};
use crate::lf::commands::util::find_repo_root;
use crate::lf::{Commands, HomeCommand};

/// `lf home <probe|start>` — the shared Home control path for any surface.
pub fn run(cmd: &HomeCommand, repo: &Path) -> anyhow::Result<()> {
    match cmd {
        HomeCommand::Probe { wave, json } => probe_cmd(wave.as_deref(), *json, repo),
        HomeCommand::Start { wave, json } => start_cmd(wave.as_deref(), *json, repo),
    }
}

fn resolve_wave_name(wave: Option<&str>) -> anyhow::Result<String> {
    wave.map(str::to_string)
        .or_else(resolve_run_wave_name)
        .ok_or_else(|| anyhow!("no wave given and none in context; pass a wave name"))
}

fn probe_cmd(wave: Option<&str>, json: bool, repo: &Path) -> anyhow::Result<()> {
    let name = resolve_wave_name(wave)?;
    let home = read_wave_home(repo, &name);
    let rt = tokio::runtime::Runtime::new()?;
    let runtime = rt.block_on(crate::ops::home::probe_home(&name, &home, repo));
    if json {
        println!("{}", serde_json::to_string(&runtime)?);
    } else {
        print_runtime(&name, &runtime);
    }
    Ok(())
}

fn start_cmd(wave: Option<&str>, json: bool, repo: &Path) -> anyhow::Result<()> {
    let name = resolve_wave_name(wave)?;
    let home = read_wave_home(repo, &name);
    let rt = tokio::runtime::Runtime::new()?;
    let result = rt
        .block_on(crate::ops::home::start_home(&name, &home, repo))
        .map_err(|error| anyhow!(error))?;
    if json {
        println!("{}", serde_json::to_string(&result)?);
    } else {
        let verb = if result.started {
            "started"
        } else {
            "already running"
        };
        println!("{name}  {verb} on {}", result.runtime.home.address);
        print_runtime(&name, &result.runtime);
    }
    Ok(())
}

fn print_runtime(name: &str, runtime: &HomeRuntimeDto) {
    let state = match runtime.state {
        HomeState::Unreachable => "unreachable",
        HomeState::Stopped => "stopped",
        HomeState::Running => "running",
        HomeState::Unknown => "unknown",
    };
    let action = match &runtime.action {
        HomeActionDto::Attach { endpoint } => format!("Attach ({endpoint})"),
        HomeActionDto::Start { home } => format!("Start on {home}"),
        HomeActionDto::Reason { message } => message.clone(),
    };
    println!("{name}  {}  [{state}]", runtime.home.address);
    println!("  reason  {}", runtime.reason);
    println!("  action  {action}");
}

/// Decide whether `command` runs on a remote home and, if so, run it there.
///
/// Returns `Some(result)` when the command was handled remotely (the caller
/// must not also run it locally), or `None` to fall through to local dispatch.
pub fn route(
    command: &Commands,
    wave: Option<&str>,
    args: &[String],
) -> Option<anyhow::Result<()>> {
    if !is_routable(command) {
        return None;
    }
    // We are already on the home host after a forward: run locally, never loop.
    if std::env::var_os(HOME_ROUTED_ENV).is_some() {
        return None;
    }
    let home = resolve_home(wave);
    // A local Home runs in-process; only a remote Home forwards.
    let dest = home.ssh_destination()?;
    Some(crate::lf::commands::ssh::run_routed(
        &dest,
        home.ssh_port(),
        None,
        &remote_argv(args),
    ))
}

/// The repo/PR/release/PM operations that must run where the Wave's work lives.
/// Deliberately minimal — everything else stays local until a concrete need
/// grows the set.
fn is_routable(command: &Commands) -> bool {
    matches!(
        command,
        Commands::Pr { .. }
            | Commands::Commit { .. }
            | Commands::Rebase { .. }
            | Commands::Release { .. }
            | Commands::Pm { .. }
    )
}

/// The Wave's Home: the pinned/inherited env value first, else the resolved
/// Wave's authored `GOAL.md`, else the current user's local Home. Only the
/// remote/local distinction matters to routing; the caller reads it off
/// [`WaveHome::ssh_destination`].
fn resolve_home(wave: Option<&str>) -> WaveHome {
    if let Ok(raw) = std::env::var(WAVE_HOME_ENV) {
        if let Some(home) = WaveHome::parse(&raw) {
            return home;
        }
    }
    let repo = find_repo_root();
    match (
        wave.map(str::to_string).or_else(resolve_run_wave_name),
        &repo,
    ) {
        (Some(name), Ok(repo)) => read_wave_home(repo, &name),
        // No wave or no repo: nothing to route — a local Home is enough.
        _ => match &repo {
            Ok(repo) => default_local_home(repo),
            Err(_) => WaveHome::local("user").expect("literal 'user' is a valid Home owner"),
        },
    }
}

/// Rebuild the invocation as an `lf` command for the remote shell: the local
/// `argv[0]` is an absolute path to this machine's binary, so replace it with
/// bare `lf` (resolved against the remote PATH) and keep every other argument.
fn remote_argv(args: &[String]) -> Vec<String> {
    let mut cmd = Vec::with_capacity(args.len());
    cmd.push("lf".to_string());
    cmd.extend(args.iter().skip(1).cloned());
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_argv_swaps_the_binary_path_for_bare_lf() {
        let args = vec![
            "/Users/jack/src/loopflow/target/debug/lf".to_string(),
            "pr".to_string(),
            "open".to_string(),
        ];
        assert_eq!(remote_argv(&args), vec!["lf", "pr", "open"]);
    }

    #[test]
    fn routable_set_is_repo_and_release_ops_only() {
        assert!(is_routable(&Commands::Commit {
            message: None,
            push: false,
            no_add: false,
        }));
        assert!(is_routable(&Commands::Pr { cmd: None }));
        assert!(!is_routable(&Commands::Status {
            wave: None,
            json: false,
        }));
        assert!(!is_routable(&Commands::Stop {
            name: "infra".to_string(),
        }));
    }
}
