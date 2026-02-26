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
    GitHubCheckRunEvent, GitHubPullRequestEvent,
};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::security::canonicalize_existing_path;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Event, Wave, WaveRun};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

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
    let repo = canonical_git_hook_repo(&payload.repo)?;
    state
        .event_hub
        .send(Event::worktree_updated(repo.clone(), repo, payload.branch));

    Ok(Json(serde_json::json!({ "ok": true })))
}

fn canonical_git_hook_repo(
    repo: &str,
) -> Result<String, (StatusCode, Json<crate::lfd::http::dto::ErrorResponse>)> {
    let path = Path::new(repo.trim());
    if !path.is_absolute() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "repo must be an absolute path",
        ));
    }

    let canonical = canonicalize_existing_path(path).map_err(|err| {
        api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Untrusted(err.to_string()),
        )
    })?;
    if !canonical.is_dir() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "repo must be an existing directory",
        ));
    }

    Ok(canonical.to_string_lossy().to_string())
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

    let event_kind = headers
        .get("X-GitHub-Event")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();

    if event_kind.eq_ignore_ascii_case("check_run") {
        let event = serde_json::from_slice::<GitHubCheckRunEvent>(&body).map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                ApiMessage::Untrusted(err.to_string()),
            )
        })?;

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
            .map_err(|err| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ApiMessage::Untrusted(err),
                )
            })?;

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

        return Ok(Json(serde_json::json!({ "ok": true, "matched": matched })));
    }

    if event_kind.eq_ignore_ascii_case("pull_request") {
        let event = serde_json::from_slice::<GitHubPullRequestEvent>(&body).map_err(|err| {
            api_error(
                StatusCode::BAD_REQUEST,
                ApiMessage::Untrusted(err.to_string()),
            )
        })?;
        if event.action != "closed" || !event.pull_request.merged {
            return Ok(Json(
                serde_json::json!({ "ok": true, "processed": 0, "skipped": true }),
            ));
        }
        let merged_at = event
            .pull_request
            .merged_at
            .as_deref()
            .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
            .unwrap_or_else(OffsetDateTime::now_utc);
        let wave_ids = find_waves_for_pr(
            &state.store,
            &event.repository.full_name,
            event.pull_request.number,
        )
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiMessage::Untrusted(err),
            )
        })?;
        let mut processed = 0_u32;
        for wave_id in wave_ids {
            let handled = crate::lfd::queue::handle_pr_merged(
                &state.store,
                &state.github,
                &wave_id,
                event.pull_request.number,
                merged_at,
            )
            .await
            .map_err(|err| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ApiMessage::Untrusted(err),
                )
            })?;
            if handled {
                processed += 1;
            }
        }
        return Ok(Json(
            serde_json::json!({ "ok": true, "processed": processed }),
        ));
    }

    Ok(Json(
        serde_json::json!({ "ok": true, "matched": 0, "skipped": true }),
    ))
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
    let waves = if let Some(wave_id) = wave_filter {
        store
            .get_wave(&wave_id)
            .await
            .map_err(|err| err.to_string())?
            .map(|wave| vec![wave])
            .unwrap_or_default()
    } else {
        store
            .list_waves(None)
            .await
            .map_err(|err| err.to_string())?
    };
    collect_wave_ci_targets(store, waves, None, None, None).await
}

async fn find_wave_ci_targets(
    store: &SharedStore,
    repo_full_name: &str,
    branch: &str,
    pr_number: Option<u32>,
) -> Result<Vec<WaveCiTarget>, String> {
    let waves = store
        .list_waves(None)
        .await
        .map_err(|err| err.to_string())?;
    collect_wave_ci_targets(store, waves, Some(repo_full_name), Some(branch), pr_number).await
}

async fn find_waves_for_pr(
    store: &SharedStore,
    repo_full_name: &str,
    pr_number: u32,
) -> Result<Vec<LfdId>, String> {
    let waves = store
        .list_waves(None)
        .await
        .map_err(|err| err.to_string())?;
    let mut matches = Vec::new();
    for wave in waves {
        let Some(repo_name) = github_repo_from_local(Path::new(wave.repo())) else {
            continue;
        };
        if repo_name != repo_full_name {
            continue;
        }
        let has_pr = store
            .list_stack_runs(wave.id())
            .await
            .map_err(|err| err.to_string())?
            .into_iter()
            .any(|run| run.snapshot.pr.and_then(|pr| pr.number) == Some(pr_number));
        if has_pr {
            matches.push(wave.id().clone());
        }
    }
    Ok(matches)
}

async fn collect_wave_ci_targets(
    store: &SharedStore,
    waves: Vec<Wave>,
    repo_filter: Option<&str>,
    branch: Option<&str>,
    pr_number: Option<u32>,
) -> Result<Vec<WaveCiTarget>, String> {
    let mut targets = Vec::new();

    for wave in waves {
        let Some(repo_full_name) = github_repo_from_local(Path::new(wave.repo())) else {
            continue;
        };
        if repo_filter.is_some_and(|repo| repo != repo_full_name) {
            continue;
        }

        let Some(target) =
            find_wave_ci_target(store, &wave, &repo_full_name, branch, pr_number).await?
        else {
            continue;
        };
        targets.push(target);
    }

    Ok(targets)
}

async fn find_wave_ci_target(
    store: &SharedStore,
    wave: &Wave,
    repo_full_name: &str,
    branch: Option<&str>,
    pr_number: Option<u32>,
) -> Result<Option<WaveCiTarget>, String> {
    let runs = store
        .list_wave_runs(Some(wave.id()), None)
        .await
        .map_err(|err| err.to_string())?;
    let run = runs
        .into_iter()
        .find(|run| run_matches_ci_target(run, branch, pr_number));

    Ok(run.and_then(|run| wave_ci_target(wave.id(), repo_full_name, &run)))
}

fn run_matches_ci_target(run: &WaveRun, branch: Option<&str>, pr_number: Option<u32>) -> bool {
    if !run.is_main() {
        return false;
    }

    let Some(pr) = run.snapshot.pr.as_ref() else {
        return false;
    };
    super::is_open_pr_state(pr.state.as_deref())
        && branch.is_none_or(|branch| pr.branch.as_deref() == Some(branch))
        && pr_number.is_none_or(|number| pr.number == Some(number))
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
    use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
    use crate::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
    use crate::lfd::events::EventHub;
    use crate::lfd::executor::WaveExecutor;
    use crate::lfd::http::state::HttpState;
    use crate::lfd::output::OutputHub;
    use crate::lfd::provider_auth::ProviderAuthService;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::sessions::SessionManager;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::{PullRequest, WaveRunKind, WaveRunSnapshot, WaveRunStatus};
    use std::sync::Arc;
    use tempfile::tempdir;
    use time::OffsetDateTime;
    use tokio::sync::Mutex;

    async fn test_http_state() -> HttpState {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let scheduler = Arc::new(Scheduler::new(1));
        let output_hub = OutputHub::new(128, tmp.path().join("output"));
        let event_hub = EventHub::new(128);
        let sessions = SessionManager::new(store.clone());
        let executor = Arc::new(
            WaveExecutor::new(
                store.clone(),
                scheduler.clone(),
                output_hub.clone(),
                event_hub.clone(),
                sessions.clone(),
                ExecutorConfig::default(),
                GitHubConfig::default(),
            )
            .expect("build executor"),
        );
        HttpState {
            store: store.clone(),
            scheduler,
            executor,
            event_hub,
            output_hub,
            provider_auth: ProviderAuthService::new(),
            auth: AuthProvider::Local {
                session_token: secrecy::SecretString::from("test-token".to_string()),
            },
            registration: None,
            started_at: OffsetDateTime::now_utc(),
            github: GitHubConfig::default(),
            http_security: HttpSecurityConfig::default(),
            auth_failure_throttle: AuthFailureThrottle::new(),
            ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
            sessions,
        }
    }

    #[tokio::test]
    async fn git_hook_handler_rejects_relative_repo_paths() {
        let state = test_http_state().await;
        let result = git_hook_handler(
            State(state),
            Json(GitHookRequest {
                hook: "post-commit".to_string(),
                repo: "../repo".to_string(),
                branch: None,
            }),
        )
        .await;
        assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
    }

    #[tokio::test]
    async fn git_hook_handler_canonicalizes_repo_path() {
        let state = test_http_state().await;
        let repo_dir = tempdir().expect("repo tempdir");
        let mut rx = state.event_hub.subscribe();
        let alias_path = repo_dir.path().join("..").join(
            repo_dir
                .path()
                .file_name()
                .expect("tempdir should have file name"),
        );
        let result = git_hook_handler(
            State(state),
            Json(GitHookRequest {
                hook: "post-commit".to_string(),
                repo: alias_path.to_string_lossy().to_string(),
                branch: Some("main".to_string()),
            }),
        )
        .await;
        assert!(result.is_ok());

        let event = rx.try_recv().expect("event emitted");
        match event {
            Event::WorktreeUpdated { repo, .. } => {
                assert_eq!(
                    repo,
                    repo_dir
                        .path()
                        .canonicalize()
                        .expect("canonical repo path")
                        .to_string_lossy()
                );
            }
            _ => panic!("expected worktree updated event"),
        }
    }

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
                flow: "build".to_string(),
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
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: "wave-group".to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
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
