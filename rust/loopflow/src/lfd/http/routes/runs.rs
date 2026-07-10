use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;

use crate::lfd::http::dto::{run_dto, ListResponse, RunDto};
use crate::lfd::http::routes::{build_wave_queue_views, resolve_wave_id};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{map_store_error, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::live_pr::{build_live_pr_snapshot, run_live_pr_key};

#[derive(Deserialize, Default)]
pub struct ListRunsQuery {
    wave_id: Option<String>,
    repo: Option<String>,
    limit: Option<u32>,
    starting_after: Option<String>,
    ending_before: Option<String>,
    order: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum RunOrder {
    #[default]
    NewestFirst,
    OldestFirst,
}

pub async fn list_runs_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListRunsQuery>,
) -> ApiResult<ListResponse<RunDto>> {
    list_runs(&state, None, query).await
}

pub async fn list_runs_for_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<ListRunsQuery>,
) -> ApiResult<ListResponse<RunDto>> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    list_runs(&state, Some(wave_id), query).await
}

async fn list_runs(
    state: &HttpState,
    path_wave_id: Option<LfdId>,
    query: ListRunsQuery,
) -> ApiResult<ListResponse<RunDto>> {
    let query_wave_id = match query.wave_id.as_deref() {
        Some(id) => Some(resolve_wave_id(state, id).await?),
        None => None,
    };
    let wave_id = path_wave_id.or(query_wave_id);
    let order = parse_run_order(query.order.as_deref());
    let stack_runs = if let Some(wave_id) = wave_id.as_ref() {
        Some(
            state
                .store
                .list_stack_runs(wave_id)
                .await
                .map_err(map_store_error)?,
        )
    } else {
        None
    };

    let runs = if let Some(wave_id) = wave_id.as_ref() {
        if order == RunOrder::OldestFirst {
            stack_runs.clone().unwrap_or_default()
        } else {
            state
                .store
                .list_runs(Some(wave_id), None)
                .await
                .map_err(map_store_error)?
        }
    } else {
        state
            .store
            .list_runs(None, None)
            .await
            .map_err(map_store_error)?
    };

    let mut filtered = runs;
    if wave_id.is_none() && order == RunOrder::OldestFirst {
        filtered.sort_by_key(|left| left.started_at);
    }

    if let Some(repo) = query.repo.as_deref() {
        filtered.retain(|run| run.repo == repo);
    }

    let (runs, has_more) = super::paginate(
        filtered,
        query.limit,
        query.starting_after.as_deref(),
        query.ending_before.as_deref(),
        |r| &r.id,
    );

    // Queue roles are global to the wave stack, so projections must use the full
    // stack regardless of pagination/order of the response slice.
    let live_snapshot = if let Some(stack_runs) = stack_runs.as_ref() {
        Some(
            build_live_pr_snapshot(&state.store, &state.github, stack_runs)
                .await
                .map_err(map_store_error)?,
        )
    } else {
        None
    };
    let queue_views =
        if let (Some(wave_id), Some(snapshot)) = (wave_id.as_ref(), live_snapshot.as_ref()) {
            Some(
                build_wave_queue_views(&state.store, wave_id, snapshot)
                    .await
                    .map_err(map_store_error)?,
            )
        } else {
            None
        };

    let mut data = Vec::with_capacity(runs.len());
    for run in runs {
        if let Some(snapshot) = live_snapshot.as_ref() {
            let live_pr_state = snapshot.state_for_run(&run);
            let pr_state_stale = snapshot.stale_for_run(&run);
            let queue_view = queue_views.as_ref().and_then(|views| views.get(&run.id));
            data.push(run_dto(run, live_pr_state, pr_state_stale, queue_view));
            continue;
        }

        let mut live_pr_state = None;
        let mut pr_state_stale = false;
        if let Some(key) = run_live_pr_key(&run) {
            live_pr_state = state
                .store
                .get_live_pr_state(&key.repo_id, key.pr_number)
                .await
                .map_err(map_store_error)?;
            pr_state_stale = live_pr_state.is_none();
        }

        data.push(run_dto(run, live_pr_state.as_ref(), pr_state_stale, None));
    }

    Ok(Json(ListResponse::new(data, has_more)))
}

fn parse_run_order(value: Option<&str>) -> RunOrder {
    match value {
        Some(value)
            if value.eq_ignore_ascii_case("oldest")
                || value.eq_ignore_ascii_case("asc")
                || value.eq_ignore_ascii_case("stack") =>
        {
            RunOrder::OldestFirst
        }
        _ => RunOrder::NewestFirst,
    }
}
