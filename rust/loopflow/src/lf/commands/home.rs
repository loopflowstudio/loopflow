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

use crate::engine::wave_config::read_wave_home;
use crate::engine::wave_context::resolve_run_wave_name;
use crate::engine::wave_home::{WaveHome, HOME_ROUTED_ENV, WAVE_HOME_ENV};
use crate::lf::commands::util::find_repo_root;
use crate::lf::Commands;

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
    let (host, repo) = match resolve_home(wave) {
        WaveHome::Local => return None,
        WaveHome::Ssh { host, repo } => (host, repo),
    };
    Some(crate::lf::commands::ssh::run_routed(
        &host,
        repo.as_deref(),
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

/// The Wave's home: the pinned/inherited env value first, else the resolved
/// Wave's authored `GOAL.md`, else `Local`.
fn resolve_home(wave: Option<&str>) -> WaveHome {
    if let Ok(raw) = std::env::var(WAVE_HOME_ENV) {
        if let Some(home) = WaveHome::parse(&raw) {
            return home;
        }
    }
    let Some(name) = wave.map(str::to_string).or_else(resolve_run_wave_name) else {
        return WaveHome::Local;
    };
    match find_repo_root() {
        Ok(repo) => read_wave_home(&repo, &name),
        Err(_) => WaveHome::Local,
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
