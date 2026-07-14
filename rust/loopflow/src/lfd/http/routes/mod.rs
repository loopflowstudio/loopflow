pub mod attention;
pub mod auth;
pub mod catalog;
pub mod flows;
pub mod hooks;
pub mod providers;
pub mod repos;
pub mod session_controls;
pub mod system;
pub mod waves;

#[cfg(test)]
pub(crate) mod test_helpers;

use crate::lfd::http::dto::{format_datetime, ErrorResponse, WaveDto};
use crate::lfd::id::LfdId;
use crate::lfd::types::Wave;
use axum::http::StatusCode;
use axum::Json;

pub type ApiError = (StatusCode, Json<ErrorResponse>);

pub fn parse_lfd_id(value: &str, error_message: &'static str) -> Result<LfdId, ApiError> {
    value
        .parse::<LfdId>()
        .map_err(|_| crate::lfd::http::api_error(StatusCode::BAD_REQUEST, error_message))
}

pub async fn resolve_wave_id(
    state: &crate::lfd::http::HttpState,
    value: &str,
) -> Result<crate::lfd::id::LfdId, (StatusCode, Json<ErrorResponse>)> {
    if let Ok(id) = value.parse::<crate::lfd::id::LfdId>() {
        return Ok(id);
    }

    let name = value.to_string();
    let wave = state
        .store
        .get_wave_by_name(&name)
        .await
        .map_err(crate::lfd::http::map_store_error)?;
    wave.map(|wave| wave.id().clone())
        .ok_or_else(|| crate::lfd::http::api_error(StatusCode::NOT_FOUND, "wave not found"))
}

pub async fn build_wave_dtos(waves: Vec<Wave>) -> Vec<WaveDto> {
    let mut views = Vec::with_capacity(waves.len());
    for wave in waves {
        views.push(build_wave_dto(wave).await);
    }
    views
}

pub async fn build_wave_dto(wave: Wave) -> WaveDto {
    let flow_name = "wave".to_string();
    let flow_repo = wave.repo().to_string();
    let flow_steps = tokio::task::spawn_blocking(move || {
        flows::load_flow_steps(&flow_name, std::path::Path::new(&flow_repo)).unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    let wave_config = crate::engine::wave_config::read_wave_config(
        std::path::Path::new(wave.repo()),
        wave.name(),
    );
    let paused = wave_config
        .as_ref()
        .and_then(|config| config.paused)
        .unwrap_or(false);
    let live = crate::wave::server::live_endpoint(std::path::Path::new(wave.repo()), wave.name())
        .await
        .is_some();
    let status = if paused {
        "paused"
    } else if live {
        "running"
    } else {
        "idle"
    };
    let goal = crate::engine::wave_config::read_wave_summary(
        std::path::Path::new(wave.repo()),
        wave.name(),
    )
    .unwrap_or_else(|_| wave.name().to_string());

    WaveDto {
        id: wave.id().to_string(),
        object: "wave".to_string(),
        name: wave.name().to_string(),
        goal,
        agent: wave_config.as_ref().and_then(|config| config.agent.clone()),
        skill_agents: wave_config.and_then(|config| config.skill_agents),
        created_at: format_datetime(wave.created_at()),
        status: status.to_string(),
        flow_steps,
        task_capacity: wave.task_capacity(),
        repo: wave.repo.clone(),
        parent_wave_id: wave.parent_wave_id().map(|id| id.to_string()),
    }
}

/// Cursor-based pagination over a list of items with an `id: LfdId` field.
pub fn paginate<T>(
    mut items: Vec<T>,
    limit: Option<u32>,
    starting_after: Option<&str>,
    ending_before: Option<&str>,
    id: fn(&T) -> &LfdId,
) -> (Vec<T>, bool) {
    if let Some(cursor) = starting_after {
        if let Some(pos) = items.iter().position(|item| id(item).as_str() == cursor) {
            items = items.split_off(pos + 1);
        }
    }
    if let Some(cursor) = ending_before {
        if let Some(pos) = items.iter().position(|item| id(item).as_str() == cursor) {
            items.truncate(pos);
        }
    }
    let mut has_more = false;
    if let Some(limit) = limit {
        let limit = limit as usize;
        if items.len() > limit {
            items.truncate(limit);
            has_more = true;
        }
    }
    (items, has_more)
}
