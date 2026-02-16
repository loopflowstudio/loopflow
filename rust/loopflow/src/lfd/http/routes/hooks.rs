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
    github_repo_from_local, poll_check_runs, verify_webhook_signature, CheckRun,
    GitHubCheckRunEvent,
};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, run_store, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::store::{RunStore, SharedStore, StoreResult};
use crate::lfd::types::{Event, Wave, WaveRun};

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

    if headers
        .get("X-GitHub-Event")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|kind| !kind.eq_ignore_ascii_case("check_run"))
    {
        return Ok(Json(
            serde_json::json!({ "ok": true, "matched": 0, "skipped": true }),
        ));
    }

    let event = serde_json::from_slice::<GitHubCheckRunEvent>(&body)
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err.to_string()))?;

    if event.action != "completed"
        || !is_failed_check_run(
            &event.check_run.status,
            event.check_run.conclusion.as_deref(),
        )
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
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err))?;

        for target in targets {
            let emitted = emit_ci_failure(
                &state.event_hub,
                &state.ci_failure_cache,
                build_ci_failure_event(&target, &event.check_run),
            )
            .await;
            if emitted {
                matched += 1;
                tracing::info!(
                    wave_id = %target.wave_id,
                    wave_run_id = %target.wave_run_id,
                    repo = %event.repository.full_name,
                    branch = %target.branch,
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
            if !is_failed_check_run(&check_run.status, check_run.conclusion.as_deref()) {
                continue;
            }

            let event = build_ci_failure_event(&target, &check_run);
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
        collect_wave_ci_targets(store, waves, None, None, None)
    })
    .await
    .map_err(|err| err.to_string())
}

async fn find_wave_ci_targets(
    store: &SharedStore,
    repo_full_name: &str,
    branch: &str,
    pr_number: Option<u32>,
) -> Result<Vec<WaveCiTarget>, String> {
    let repo_full_name = repo_full_name.to_string();
    let branch = branch.to_string();
    run_store(store, move |store| {
        let waves = store.list_waves(None)?;
        collect_wave_ci_targets(
            store,
            waves,
            Some(repo_full_name.as_str()),
            Some(branch.as_str()),
            pr_number,
        )
    })
    .await
    .map_err(|err| err.to_string())
}

fn collect_wave_ci_targets(
    store: &dyn RunStore,
    waves: Vec<Wave>,
    repo_filter: Option<&str>,
    branch: Option<&str>,
    pr_number: Option<u32>,
) -> StoreResult<Vec<WaveCiTarget>> {
    let mut targets = Vec::new();

    for wave in waves {
        let Some(repo_full_name) = github_repo_from_local(Path::new(&wave.repo)) else {
            continue;
        };
        if repo_filter.is_some_and(|repo| repo != repo_full_name) {
            continue;
        }

        let Some(target) = find_wave_ci_target(store, &wave, &repo_full_name, branch, pr_number)?
        else {
            continue;
        };
        targets.push(target);
    }

    Ok(targets)
}

fn find_wave_ci_target(
    store: &dyn RunStore,
    wave: &Wave,
    repo_full_name: &str,
    branch: Option<&str>,
    pr_number: Option<u32>,
) -> StoreResult<Option<WaveCiTarget>> {
    let run = store
        .list_wave_runs(Some(&wave.id), None)?
        .into_iter()
        .find(|run| run_matches_ci_target(run, branch, pr_number));

    Ok(run.and_then(|run| wave_ci_target(&wave.id, repo_full_name, &run)))
}

fn run_matches_ci_target(run: &WaveRun, branch: Option<&str>, pr_number: Option<u32>) -> bool {
    if !run.is_main() {
        return false;
    }

    let Some(pr) = run.snapshot.pr.as_ref() else {
        return false;
    };
    if !super::is_open_pr_state(pr.state.as_deref()) {
        return false;
    }
    if let Some(branch) = branch {
        if pr.branch.as_deref() != Some(branch) {
            return false;
        }
    }
    if let Some(pr_number) = pr_number {
        if pr.number != Some(pr_number) {
            return false;
        }
    }

    true
}

fn is_failed_check_run(status: &str, conclusion: Option<&str>) -> bool {
    status == "completed" && conclusion == Some("failure")
}

fn wave_ci_target(wave_id: &LfdId, repo_full_name: &str, run: &WaveRun) -> Option<WaveCiTarget> {
    let pr = run.snapshot.pr.as_ref()?;
    Some(WaveCiTarget {
        wave_id: wave_id.clone(),
        wave_run_id: run.id.clone(),
        repo_full_name: repo_full_name.to_string(),
        branch: pr.branch.clone()?,
        pr_number: pr.number?,
    })
}

fn build_ci_failure_event(target: &WaveCiTarget, check_run: &CheckRun) -> Event {
    Event::ci_failure(
        target.wave_id.clone(),
        target.wave_run_id.clone(),
        target.pr_number,
        target.branch.clone(),
        check_run.head_sha.clone(),
        check_run.name.clone(),
        check_run.html_url.clone(),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::types::{PullRequest, WaveRunKind, WaveRunSnapshot, WaveRunStatus};

    fn wave_run_with_pr(
        run_kind: WaveRunKind,
        pr_state: Option<&str>,
        branch: Option<&str>,
    ) -> WaveRun {
        WaveRun {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            snapshot: WaveRunSnapshot {
                repo: ".".to_string(),
                flow: "ship".to_string(),
                direction: Vec::new(),
                area: Vec::new(),
                pr: Some(PullRequest {
                    url: "https://example.test/pr/1".to_string(),
                    number: Some(1),
                    state: pr_state.map(ToString::to_string),
                    title: Some("test".to_string()),
                    branch: branch.map(ToString::to_string),
                }),
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: "/tmp/worktree".to_string(),
            branch: "feature".to_string(),
            started_at: None,
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind,
            sidecar_kind: None,
        }
    }

    #[test]
    fn run_matches_ci_target_only_matches_open_main_prs() {
        let run = wave_run_with_pr(WaveRunKind::Main, Some("open"), Some("feature"));
        assert!(run_matches_ci_target(&run, Some("feature"), Some(1)));

        let closed = wave_run_with_pr(WaveRunKind::Main, Some("closed"), Some("feature"));
        assert!(!run_matches_ci_target(&closed, Some("feature"), Some(1)));

        let unknown_state = wave_run_with_pr(WaveRunKind::Main, None, Some("feature"));
        assert!(!run_matches_ci_target(
            &unknown_state,
            Some("feature"),
            Some(1)
        ));

        let sidecar = wave_run_with_pr(WaveRunKind::Sidecar, Some("open"), Some("feature"));
        assert!(!run_matches_ci_target(&sidecar, Some("feature"), Some(1)));
    }

    #[tokio::test]
    async fn emit_ci_failure_deduplicates_by_wave_and_commit_sha() {
        let event_hub = EventHub::new(8);
        let cache = Arc::new(Mutex::new(HashSet::new()));
        let event = Event::ci_failure(
            LfdId::new(),
            LfdId::new(),
            1,
            "feature".to_string(),
            "abc123".to_string(),
            "test-check".to_string(),
            "https://example.test/logs".to_string(),
        );

        assert!(emit_ci_failure(&event_hub, &cache, event.clone()).await);
        assert!(!emit_ci_failure(&event_hub, &cache, event).await);
    }
}
