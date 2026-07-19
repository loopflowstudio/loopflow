//! Probe one durable Home without owning a second lifecycle or Home model.

use std::path::Path;

use crate::durable::Home;
use crate::engine::wave_home::{resolve_home_relative_repo, HomeRoute, HomeRuntimeDto, HomeState};
use crate::lf::commands::ssh::{capture_remote_native, SshCaptureError};
use crate::wave::server::live_endpoint;

pub async fn probe_home(wave: &str, home: &Home, repo: &Path) -> HomeRuntimeDto {
    if home.route == "local" {
        return probe_local(wave, home, repo).await;
    }
    let Some(route) = HomeRoute::parse(&home.route).filter(HomeRoute::is_remote) else {
        return HomeRuntimeDto::new(
            home,
            HomeState::Unknown,
            format!("Home {} has invalid route {:?}", home.id, home.route),
            None,
        );
    };
    probe_remote(wave, home, &route, repo).await
}

async fn probe_local(wave: &str, home: &Home, repo: &Path) -> HomeRuntimeDto {
    match live_endpoint(repo, wave).await {
        Some(endpoint) => HomeRuntimeDto::new(
            home,
            HomeState::Running,
            "resident is serving on this Home".to_string(),
            Some(endpoint),
        ),
        None => HomeRuntimeDto::new(
            home,
            HomeState::Stopped,
            "Home is reachable; no resident is serving this Wave".to_string(),
            None,
        ),
    }
}

async fn probe_remote(wave: &str, home: &Home, _route: &HomeRoute, repo: &Path) -> HomeRuntimeDto {
    let remote_repo = match resolve_home_relative_repo(repo) {
        Ok(repo) => repo,
        Err(reason) => return HomeRuntimeDto::new(home, HomeState::Unknown, reason, None),
    };
    let home_id = home.id.clone();
    let wave = wave.to_string();
    let cmd = vec![
        "lf".to_string(),
        "status".to_string(),
        wave,
        "--json".to_string(),
    ];
    let captured =
        tokio::task::spawn_blocking(move || capture_remote_native(&home_id, &remote_repo, &cmd))
            .await;
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
        Err(error) => HomeRuntimeDto::new(
            home,
            HomeState::Unknown,
            format!("Home probe task failed: {error}"),
            None,
        ),
    }
}

fn classify_remote_status(home: &Home, stdout: &str) -> HomeRuntimeDto {
    let value: serde_json::Value = match serde_json::from_str(stdout.trim()) {
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
    if value.is_null() {
        return HomeRuntimeDto::new(
            home,
            HomeState::Stopped,
            "Home is reachable; the Wave is not running".to_string(),
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
            "Home is reachable; no resident is serving this Wave".to_string(),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable::HomeId;

    fn home() -> Home {
        Home {
            id: HomeId::parse("home_00000000000000000000000000000001").unwrap(),
            route: "ssh://jack@box".to_string(),
            created_at: time::OffsetDateTime::UNIX_EPOCH,
            observed_at: time::OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn remote_status_live_endpoint_is_running_with_attach_identity() {
        let runtime = classify_remote_status(
            &home(),
            r#"{"wave":{"live":true,"endpoint":"127.0.0.1:7777"}}"#,
        );
        assert_eq!(runtime.state, HomeState::Running);
        assert_eq!(runtime.endpoint.as_deref(), Some("127.0.0.1:7777"));
    }

    #[test]
    fn remote_status_not_live_is_stopped() {
        let runtime = classify_remote_status(&home(), r#"{"wave":{"live":false,"endpoint":null}}"#);
        assert_eq!(runtime.state, HomeState::Stopped);
        assert!(runtime.endpoint.is_none());
    }

    #[test]
    fn remote_null_registry_is_reachable_but_stopped() {
        assert_eq!(
            classify_remote_status(&home(), "null").state,
            HomeState::Stopped
        );
    }

    #[test]
    fn remote_garbage_is_unknown_not_stopped() {
        assert_eq!(
            classify_remote_status(&home(), "not json").state,
            HomeState::Unknown
        );
    }
}
