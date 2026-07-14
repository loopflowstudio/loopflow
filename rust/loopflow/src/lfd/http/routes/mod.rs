pub mod attention;
pub mod auth;
pub mod catalog;
pub mod flows;
pub mod hooks;
pub mod providers;
pub mod repos;
pub mod runs;
pub mod session_controls;
pub mod system;
pub mod waves;

#[cfg(test)]
pub(crate) mod test_helpers;

use crate::lfd::http::dto::{format_datetime, run_dto, ErrorResponse, WaveDto};
use crate::lfd::id::LfdId;
use crate::lfd::types::{Run, Wave, DEFAULT_WAVE_FLOW};
use crate::lfdb::{SharedStore, StoreError};
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

pub async fn build_wave_dtos(
    store: &SharedStore,
    waves: Vec<Wave>,
    include_active_run: bool,
) -> Result<Vec<WaveDto>, StoreError> {
    let mut views = Vec::with_capacity(waves.len());
    for wave in waves {
        views.push(build_wave_dto(store, wave, include_active_run).await?);
    }
    Ok(views)
}

pub async fn build_wave_dto(
    store: &SharedStore,
    wave: Wave,
    include_active_run: bool,
) -> Result<WaveDto, StoreError> {
    let stored_runs = store.list_runs(Some(wave.id()), None).await?;

    let flow_name = DEFAULT_WAVE_FLOW.to_string();
    let flow_repo = wave.repo().to_string();
    let flow_steps = tokio::task::spawn_blocking(move || {
        flows::load_flow_steps(&flow_name, std::path::Path::new(&flow_repo)).unwrap_or_default()
    })
    .await
    .unwrap_or_default();

    // Runs are execution history, not Wave delivery. A Wave has no branch,
    // worktree, diff, or PR; only Task Sessions own those shipping fields.
    let repo_runs: Vec<Run> = stored_runs
        .iter()
        .filter(|run| run.repo == wave.repo)
        .cloned()
        .collect();
    let latest_for_repo = repo_runs.iter().max_by_key(|run| run.started_at);
    let active_run = if include_active_run {
        latest_for_repo.map(|run| run_dto(run.clone(), None, false))
    } else {
        None
    };
    let wave_config = crate::engine::wave_config::read_wave_config(
        std::path::Path::new(wave.repo()),
        wave.name(),
    );

    Ok(WaveDto {
        id: wave.id().to_string(),
        object: "wave".to_string(),
        name: wave.name().clone(),
        goal: wave.goal().to_string(),
        metrics: wave.metrics().clone(),
        direction: wave.direction().clone(),
        area: wave.area().clone(),
        agent: wave_config.as_ref().and_then(|config| config.agent.clone()),
        skill_agents: wave_config.and_then(|config| config.skill_agents),
        created_at: format_datetime(wave.created_at()),
        status: wave.status().as_str().to_string(),
        flow_steps,
        workers: wave.workers(),
        repo: wave.repo.clone(),
        iteration: wave.iteration(),
        active_run,
        parent_wave_id: wave.parent_wave_id().map(|id| id.to_string()),
    })
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

pub(crate) fn is_open_pr_state(state: Option<&str>) -> bool {
    match state {
        Some(state) => state.eq_ignore_ascii_case("open") || state.eq_ignore_ascii_case("draft"),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Run, RunStatus, Wave, WaveStatus};
    use crate::lfdb::SharedStore;
    use std::sync::Arc;
    use time::OffsetDateTime;

    async fn sqlite_store() -> SharedStore {
        let db_path = std::env::temp_dir().join(format!("lfd-routes-test-{}.db", LfdId::new()));
        let config = crate::lfdb::StorageConfig::sqlite(db_path);
        Arc::new(
            crate::lfdb::open_store(&config)
                .await
                .expect("sqlite store should initialize"),
        )
    }

    fn make_wave(repo: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: "wave-runtime".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: repo.to_string(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: vec![],
            area: vec![],
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_run(wave: &Wave, iteration: u32) -> Run {
        Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: wave.repo().to_string(),
            flow: DEFAULT_WAVE_FLOW.to_string(),
            task: None,
            direction: wave.direction().clone(),
            area: wave.area().clone(),
            iteration,
            step_index: 0,
            status: RunStatus::Completed,
            worktree: "/tmp/worktree".to_string(),
            branch: String::new(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: Some(OffsetDateTime::now_utc()),
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            parent_run_id: None,
            repair_of: None,
            pr: None,
        }
    }

    #[test]
    fn unknown_pr_state_is_not_open() {
        assert!(!is_open_pr_state(None));
        assert!(!is_open_pr_state(Some("closed")));
        assert!(!is_open_pr_state(Some("merged")));
        assert!(is_open_pr_state(Some("open")));
        assert!(is_open_pr_state(Some("draft")));
    }

    #[tokio::test]
    async fn build_wave_dto_selects_latest_run_for_repo() {
        let store = sqlite_store().await;
        let repo = tempfile::tempdir().expect("repo");
        let repo_path = repo.path().to_str().expect("repo path").to_string();
        let started = OffsetDateTime::now_utc();

        let wave = make_wave(&repo_path);
        store
            .create_wave(&wave)
            .await
            .expect("wave should be created in store");

        let mut run_old = make_run(&wave, 11);
        run_old.repo = repo_path.clone();
        run_old.started_at = Some(started);
        let mut run_new = make_run(&wave, 22);
        run_new.repo = repo_path;
        run_new.started_at = Some(started + time::Duration::seconds(1));
        store
            .create_run(&run_old)
            .await
            .expect("old run should be created");
        store
            .create_run(&run_new)
            .await
            .expect("new run should be created");

        let dto = build_wave_dto(&store, wave, true)
            .await
            .expect("wave dto should be built");

        assert_eq!(
            dto.active_run.as_ref().map(|run| run.iteration),
            Some(22),
            "the latest run by start time wins"
        );
    }
}
