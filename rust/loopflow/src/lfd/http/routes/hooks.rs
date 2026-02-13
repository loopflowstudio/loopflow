use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use bytes::Bytes;
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::lfd::events::EventHub;
use crate::lfd::github::{
    github_repo_from_local, poll_check_runs, verify_webhook_signature, GitHubCheckRunEvent,
};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, run_store, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Event, WaveRun};

#[derive(Deserialize)]
pub struct GitHookRequest {
    #[allow(dead_code)]
    hook: String,
    repo: String,
    branch: Option<String>,
}

#[derive(Debug, Clone)]
struct WaveCiTarget {
    wave_id: LfdId,
    wave_run_id: LfdId,
    repo_full_name: String,
    branch: String,
    pr_number: u32,
}

pub async fn git_hook_handler(
    State(state): State<HttpState>,
    Json(payload): Json<GitHookRequest>,
) -> ApiResult<serde_json::Value> {
    state.event_hub.send(Event::worktree_updated(
        payload.repo.clone(),
        payload.repo,
        payload.branch,
    ));

    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn github_webhook_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<serde_json::Value> {
    let secret = state.github.webhook_secret.clone();
    if secret.trim().is_empty() {
        return Err(api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "github webhook secret is not configured",
        ));
    }

    let signature = headers
        .get("X-Hub-Signature-256")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "missing X-Hub-Signature-256"))?;
    if !verify_webhook_signature(&secret, body.as_ref(), signature) {
        return Err(api_error(
            StatusCode::UNAUTHORIZED,
            "invalid webhook signature",
        ));
    }

    let event = serde_json::from_slice::<GitHubCheckRunEvent>(&body)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err.to_string()))?;

    if event.action != "completed"
        || event.check_run.status != "completed"
        || event.check_run.conclusion.as_deref() != Some("failure")
    {
        return Ok(Json(
            serde_json::json!({ "ok": true, "matched": 0, "skipped": true }),
        ));
    }

    let mut matched = 0_u32;
    for pr in &event.check_run.pull_requests {
        let targets = find_wave_ci_targets(
            &state.store,
            &event.repository.full_name,
            &pr.head.branch,
            Some(pr.number),
        )
        .await?;

        for target in targets {
            let wave_run_id = target.wave_run_id.clone();
            let emitted = emit_ci_failure(
                &state.event_hub,
                &state.ci_failure_cache,
                Event::ci_failure(
                    target.wave_id.clone(),
                    wave_run_id,
                    pr.number,
                    pr.head.branch.clone(),
                    event.check_run.head_sha.clone(),
                    event.check_run.name.clone(),
                    event.check_run.html_url.clone(),
                ),
            )
            .await;
            if emitted {
                matched += 1;
                tracing::info!(
                    wave_id = %target.wave_id,
                    wave_run_id = %target.wave_run_id,
                    repo = %event.repository.full_name,
                    branch = %pr.head.branch,
                    commit_sha = %event.check_run.head_sha,
                    check_id = event.check_run.id,
                    check_name = %event.check_run.name,
                    "matched GitHub CI failure to wave"
                );
            }
        }
    }

    Ok(Json(serde_json::json!({ "ok": true, "matched": matched })))
}

pub async fn poll_all_waves_ci(
    store: &SharedStore,
    event_hub: &EventHub,
    token: &str,
    cache: &Arc<Mutex<HashSet<String>>>,
) -> Result<u32, String> {
    let targets = list_wave_ci_targets(store, None).await?;
    emit_ci_failures_for_targets(event_hub, cache, token, targets).await
}

pub async fn poll_wave_ci(
    store: &SharedStore,
    event_hub: &EventHub,
    cache: &Arc<Mutex<HashSet<String>>>,
    wave_id: &LfdId,
    token: &str,
) -> Result<u32, String> {
    let targets = list_wave_ci_targets(store, Some(wave_id.clone())).await?;
    emit_ci_failures_for_targets(event_hub, cache, token, targets).await
}

async fn emit_ci_failures_for_targets(
    event_hub: &EventHub,
    cache: &Arc<Mutex<HashSet<String>>>,
    token: &str,
    targets: Vec<WaveCiTarget>,
) -> Result<u32, String> {
    let mut emitted = 0_u32;

    for target in targets {
        let check_runs = poll_check_runs(&target.repo_full_name, &target.branch, token).await?;
        for check_run in check_runs {
            if check_run.status != "completed" || check_run.conclusion.as_deref() != Some("failure")
            {
                continue;
            }

            let event = Event::ci_failure(
                target.wave_id.clone(),
                target.wave_run_id.clone(),
                target.pr_number,
                target.branch.clone(),
                check_run.head_sha.clone(),
                check_run.name.clone(),
                check_run.html_url.clone(),
            );
            if emit_ci_failure(event_hub, cache, event).await {
                emitted += 1;
            }
        }
    }

    Ok(emitted)
}

async fn list_wave_ci_targets(
    store: &SharedStore,
    wave_filter: Option<LfdId>,
) -> Result<Vec<WaveCiTarget>, String> {
    run_store(store, move |store| {
        let waves = if let Some(wave_id) = wave_filter {
            store
                .get_wave(&wave_id)?
                .map(|wave| vec![wave])
                .unwrap_or_default()
        } else {
            store.list_waves(None)?
        };

        let mut targets = Vec::new();
        for wave in waves {
            let Some(repo_full_name) = github_repo_from_local(Path::new(&wave.repo)) else {
                continue;
            };
            let runs = store.list_wave_runs(Some(&wave.id), None)?;
            let Some(run) = select_run_with_open_pr(runs) else {
                continue;
            };
            let Some(pr) = run.snapshot.pr.as_ref() else {
                continue;
            };
            let Some(branch) = pr.branch.clone() else {
                continue;
            };
            let Some(pr_number) = pr.number else {
                continue;
            };

            targets.push(WaveCiTarget {
                wave_id: wave.id.clone(),
                wave_run_id: run.id.clone(),
                repo_full_name,
                branch,
                pr_number,
            });
        }
        Ok(targets)
    })
    .await
    .map_err(|err| err.to_string())
}

async fn find_wave_ci_targets(
    store: &SharedStore,
    repo_full_name: &str,
    branch: &str,
    pr_number: Option<u32>,
) -> Result<Vec<WaveCiTarget>, (StatusCode, Json<crate::lfd::http::dto::ErrorResponse>)> {
    let repo_full_name = repo_full_name.to_string();
    let branch = branch.to_string();
    run_store(store, move |store| {
        let waves = store.list_waves(None)?;
        let mut targets = Vec::new();

        for wave in waves {
            let Some(wave_repo) = github_repo_from_local(Path::new(&wave.repo)) else {
                continue;
            };
            if wave_repo != repo_full_name {
                continue;
            }

            let runs = store.list_wave_runs(Some(&wave.id), None)?;
            let Some(run) = runs.into_iter().find(|run| {
                run.is_main()
                    && run.snapshot.pr.as_ref().is_some_and(|pr| {
                        pr.branch.as_deref() == Some(branch.as_str())
                            && pr_number.is_none_or(|number| pr.number == Some(number))
                            && is_open_pr_state(pr.state.as_deref())
                    })
            }) else {
                continue;
            };

            let Some(pr) = run.snapshot.pr.as_ref() else {
                continue;
            };
            let Some(target_pr_number) = pr.number else {
                continue;
            };
            targets.push(WaveCiTarget {
                wave_id: wave.id.clone(),
                wave_run_id: run.id.clone(),
                repo_full_name: wave_repo,
                branch: branch.clone(),
                pr_number: target_pr_number,
            });
        }

        Ok(targets)
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

fn select_run_with_open_pr(runs: Vec<WaveRun>) -> Option<WaveRun> {
    runs.into_iter().find(|run| {
        run.is_main()
            && run
                .snapshot
                .pr
                .as_ref()
                .is_some_and(|pr| is_open_pr_state(pr.state.as_deref()))
    })
}

fn is_open_pr_state(state: Option<&str>) -> bool {
    match state {
        Some(value) => value.eq_ignore_ascii_case("open") || value.eq_ignore_ascii_case("draft"),
        None => true,
    }
}

async fn emit_ci_failure(
    event_hub: &EventHub,
    cache: &Arc<Mutex<HashSet<String>>>,
    event: Event,
) -> bool {
    let (wave_id, commit_sha) = match &event {
        Event::CiFailure {
            wave_id,
            commit_sha,
            ..
        } => (wave_id.clone(), commit_sha.clone()),
        _ => return false,
    };

    let key = format!("{wave_id}:{commit_sha}");
    let mut cache = cache.lock().await;
    if !cache.insert(key) {
        return false;
    }
    event_hub.send(event);
    true
}
