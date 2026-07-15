//! The shared Home control path: resolve a Wave to its Home, probe that Home for
//! liveness with evidence, and idempotently start the Wave on its Home.
//!
//! Every surface — CLI, and the conductor UI via `lf home … --json` — uses this
//! one path. Probe and start route through the Home address and the existing
//! `lf ssh` credential machinery (`capture_routed`); no surface reimplements SSH,
//! infers the remote host, or owns a parallel lifecycle. Starting targets the
//! configured Home, not the machine running the caller, and is safe to repeat:
//! it probes first and returns the running resident's identity instead of
//! launching a second one.

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::engine::process::{resolve_lf_binary, start_lf_session, tmux_session_slug};
use crate::engine::wave_home::{HomeLocation, HomeRuntimeDto, HomeState, WaveHome, WaveHomeDto};
use crate::lf::commands::ssh::{capture_routed, SshCaptureError};
use crate::wave::server::live_endpoint;

/// The result of `lf home start`: the Home, the Wave, whether this call launched
/// the resident (vs. found it already running), and the post-start evidence
/// carrying the attach identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HomeStartResult {
    pub wave: String,
    pub started: bool,
    pub runtime: HomeRuntimeDto,
}

/// Probe a Wave's Home and classify what is happening there, with evidence.
///
/// Local Homes are read directly; remote Homes route a `lf status --json` through
/// the Home's SSH address. The returned [`HomeRuntimeDto`] carries the state, the
/// reason (evidence), the attach endpoint when running, and the one contextual
/// action a surface should offer.
pub async fn probe_home(wave: &str, home: &WaveHome, repo: &Path) -> HomeRuntimeDto {
    match home.location() {
        HomeLocation::Local => probe_local(wave, home, repo).await,
        HomeLocation::Remote { .. } => probe_remote(wave, home).await,
    }
}

async fn probe_local(wave: &str, home: &WaveHome, repo: &Path) -> HomeRuntimeDto {
    match live_endpoint(repo, wave).await {
        Some(endpoint) => HomeRuntimeDto::new(
            home,
            HomeState::Running,
            "resident is serving on this machine".to_string(),
            Some(endpoint),
        ),
        None => HomeRuntimeDto::new(
            home,
            HomeState::Stopped,
            "reachable (local); no resident is serving this Wave".to_string(),
            None,
        ),
    }
}

async fn probe_remote(wave: &str, home: &WaveHome) -> HomeRuntimeDto {
    let dest = home
        .ssh_destination()
        .expect("a remote Home always yields an ssh destination");
    let port = home.ssh_port();
    let wave_arg = wave.to_string();
    let cmd = vec![
        "lf".to_string(),
        "status".to_string(),
        wave_arg,
        "--json".to_string(),
    ];
    // `capture_routed` builds its own runtime; run it off the async worker.
    let captured = tokio::task::spawn_blocking(move || capture_routed(&dest, port, &cmd)).await;
    match captured {
        Ok(Ok(stdout)) => classify_remote_status(home, &stdout),
        Ok(Err(SshCaptureError::Unreachable(reason))) => {
            HomeRuntimeDto::new(home, HomeState::Unreachable, reason, None)
        }
        Ok(Err(SshCaptureError::Command { code, stderr })) => HomeRuntimeDto::new(
            home,
            HomeState::Unknown,
            format!("Home answered but `lf status` exited {code}: {stderr}"),
            None,
        ),
        Ok(Err(SshCaptureError::Local(reason))) => {
            HomeRuntimeDto::new(home, HomeState::Unknown, reason, None)
        }
        Err(join) => HomeRuntimeDto::new(
            home,
            HomeState::Unknown,
            format!("Home probe task failed: {join}"),
            None,
        ),
    }
}

/// Read a remote `lf status --json` payload into runtime evidence. A parseable
/// snapshot with a live endpoint is Running; a reachable Home with no live
/// resident is Stopped; anything else is Unknown.
fn classify_remote_status(home: &WaveHome, stdout: &str) -> HomeRuntimeDto {
    let trimmed = stdout.trim();
    let value: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(value) => value,
        Err(error) => {
            return HomeRuntimeDto::new(
                home,
                HomeState::Unknown,
                format!("Home answered but its status was unreadable: {error}"),
                None,
            );
        }
    };
    // `lf status` prints `null` when the Home has no registry — reachable, but
    // nothing is running there.
    if value.is_null() {
        return HomeRuntimeDto::new(
            home,
            HomeState::Stopped,
            "reachable; the Home has no running Wave".to_string(),
            None,
        );
    }
    let wave = value.get("wave");
    let live = wave
        .and_then(|wave| wave.get("live"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let endpoint = wave
        .and_then(|wave| wave.get("endpoint"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    match (live, endpoint) {
        (true, Some(endpoint)) => HomeRuntimeDto::new(
            home,
            HomeState::Running,
            "resident is serving on the Home".to_string(),
            Some(endpoint),
        ),
        _ => HomeRuntimeDto::new(
            home,
            HomeState::Stopped,
            "reachable; no resident is serving this Wave".to_string(),
            None,
        ),
    }
}

/// Idempotently start a Wave on its configured Home and return the attach
/// identity. Probes first: an already-running Home returns its resident without
/// launching a second. A reachable-but-stopped Home is started on the Home
/// (local: a detached tmux `lf wave`; remote: the same over SSH); an unreachable
/// Home is not started and its reason is returned. After launching, the Home is
/// re-probed so the result carries the resident's endpoint when it has bound.
pub async fn start_home(
    wave: &str,
    home: &WaveHome,
    repo: &Path,
) -> Result<HomeStartResult, String> {
    let probed = probe_home(wave, home, repo).await;
    match probed.state {
        HomeState::Running => {
            return Ok(HomeStartResult {
                wave: wave.to_string(),
                started: false,
                runtime: probed,
            })
        }
        HomeState::Unreachable => {
            return Err(format!(
                "cannot start Wave '{wave}' on {}: {}",
                home, probed.reason
            ))
        }
        // Stopped or Unknown: attempt the start; the re-probe reports the truth.
        HomeState::Stopped | HomeState::Unknown => {}
    }

    match home.location() {
        HomeLocation::Local => start_local(wave, repo).await?,
        HomeLocation::Remote { .. } => start_remote(wave, home).await?,
    }

    let runtime = probe_until_running(wave, home, repo).await;
    Ok(HomeStartResult {
        wave: wave.to_string(),
        started: true,
        runtime,
    })
}

/// Launch `lf wave <wave>` in a detached tmux session on this machine — the same
/// door a human uses from a terminal, so quitting any UI never kills the Wave.
async fn start_local(wave: &str, repo: &Path) -> Result<(), String> {
    let argv = vec![
        resolve_lf_binary().to_string_lossy().to_string(),
        "wave".to_string(),
        wave.to_string(),
    ];
    start_lf_session(&wave_session_name(repo, wave), repo, &argv)
        .await
        .map_err(|error| format!("failed to start Wave '{wave}' locally: {error}"))
}

/// Launch `lf wave <wave>` on the remote Home in a detached tmux session, routed
/// through the Home's SSH address. Uses `capture_routed` so a transport failure
/// is an error rather than a process exit.
async fn start_remote(wave: &str, home: &WaveHome) -> Result<(), String> {
    let dest = home
        .ssh_destination()
        .expect("a remote Home always yields an ssh destination");
    let port = home.ssh_port();
    let session = wave_session_name(Path::new(crate::lf::commands::ssh::DEFAULT_REPO), wave);
    let wave_arg = wave.to_string();
    let cmd = vec![
        "tmux".to_string(),
        "new-session".to_string(),
        "-d".to_string(),
        "-s".to_string(),
        session,
        "lf".to_string(),
        "wave".to_string(),
        wave_arg,
    ];
    tokio::task::spawn_blocking(move || capture_routed(&dest, port, &cmd))
        .await
        .map_err(|join| format!("Home start task failed: {join}"))?
        .map(|_| ())
        .map_err(|error| match error {
            SshCaptureError::Unreachable(reason) => {
                format!("cannot reach Home to start Wave '{wave}': {reason}")
            }
            SshCaptureError::Command { code, stderr } => {
                format!("Home rejected the start of Wave '{wave}' (exit {code}): {stderr}")
            }
            SshCaptureError::Local(reason) => reason,
        })
}

/// A stable tmux session name for a Wave's resident, matching the residency
/// naming so a repeat start reuses one session rather than stacking.
fn wave_session_name(repo: &Path, wave: &str) -> String {
    format!(
        "lf-{}-{}",
        tmux_session_slug(&repo.display().to_string()),
        tmux_session_slug(wave)
    )
}

/// Re-probe a few times so a resident that is still binding its endpoint is
/// reported Running with its attach identity, not prematurely Stopped.
async fn probe_until_running(wave: &str, home: &WaveHome, repo: &Path) -> HomeRuntimeDto {
    let mut last = probe_home(wave, home, repo).await;
    for _ in 0..4 {
        if last.state == HomeState::Running {
            return last;
        }
        tokio::time::sleep(Duration::from_millis(750)).await;
        last = probe_home(wave, home, repo).await;
    }
    last
}

/// The wire-facing Home for a Wave, for `lf home probe`/`start` output.
pub fn home_dto(home: &WaveHome) -> WaveHomeDto {
    WaveHomeDto::from(home)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(raw: &str) -> WaveHome {
        WaveHome::parse(raw).unwrap()
    }

    #[test]
    fn remote_status_live_endpoint_is_running_with_attach_identity() {
        let json = r#"{"wave":{"live":true,"endpoint":"127.0.0.1:7777"}}"#;
        let runtime = classify_remote_status(&home("ssh://jack@box"), json);
        assert_eq!(runtime.state, HomeState::Running);
        assert_eq!(runtime.endpoint.as_deref(), Some("127.0.0.1:7777"));
    }

    #[test]
    fn remote_status_not_live_is_stopped_with_start_action() {
        let json = r#"{"wave":{"live":false,"endpoint":null}}"#;
        let runtime = classify_remote_status(&home("ssh://jack@box"), json);
        assert_eq!(runtime.state, HomeState::Stopped);
        assert!(runtime.endpoint.is_none());
    }

    #[test]
    fn remote_null_registry_is_reachable_but_stopped() {
        let runtime = classify_remote_status(&home("ssh://jack@box"), "null");
        assert_eq!(runtime.state, HomeState::Stopped);
    }

    #[test]
    fn remote_garbage_is_unknown_not_stopped() {
        let runtime = classify_remote_status(&home("ssh://jack@box"), "not json");
        assert_eq!(runtime.state, HomeState::Unknown);
    }

    #[test]
    fn wave_session_name_is_stable_and_slugged() {
        let a = wave_session_name(Path::new("/src/loopflow"), "infrastructure");
        let b = wave_session_name(Path::new("/src/loopflow"), "infrastructure");
        assert_eq!(a, b);
        assert!(a.starts_with("lf-"));
        assert!(a.ends_with("-infrastructure"));
    }
}
