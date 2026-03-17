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
    GitHubCheckRunEvent, GitHubPullRequestEvent, GitHubPushEvent,
};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::security::canonicalize_existing_path;
use crate::lfd::store::SharedStore;
use crate::lfd::triggers::{enqueue_pending_activation, ActivationEnvelope};
use crate::lfd::types::{Event, Signal, Trigger, Wave, WaveRun, WaveStatus, CI_FIX_FLOW};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

#[derive(Deserialize)]
pub struct GitHookRequest {
    #[allow(dead_code)]
    hook: String,
    repo: String,
    branch: Option<String>,
    from_sha: Option<String>,
    to_sha: Option<String>,
}

#[derive(Debug, Clone)]
struct WaveCiTarget {
    wave_id: LfdId,
    wave_run_id: LfdId,
    repo_full_name: String,
    branch: String,
    pr_number: u32,
}

#[derive(Debug, Clone, Copy)]
enum WatchRepoTarget<'a> {
    LocalPath(&'a str),
    GitHubRepo(&'a str),
}

impl WatchRepoTarget<'_> {
    fn label(self) -> &'static str {
        match self {
            Self::LocalPath(_) => "git hook",
            Self::GitHubRepo(_) => "github push",
        }
    }
}

pub async fn git_hook_handler(
    State(state): State<HttpState>,
    Json(payload): Json<GitHookRequest>,
) -> ApiResult<serde_json::Value> {
    let repo = canonical_git_hook_repo(&payload.repo)?;
    let matched = enqueue_watch_for_local_repo(
        &state.store,
        &state.event_hub,
        &repo,
        payload.branch.as_deref(),
        payload.from_sha.as_deref(),
        payload.to_sha.as_deref(),
    )
    .await;
    state
        .event_hub
        .send(Event::worktree_updated(repo.clone(), repo, payload.branch));

    Ok(Json(serde_json::json!({ "ok": true, "matched": matched })))
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

async fn enqueue_watch_for_local_repo(
    store: &SharedStore,
    event_hub: &EventHub,
    repo_path: &str,
    branch: Option<&str>,
    from_sha: Option<&str>,
    to_sha: Option<&str>,
) -> u32 {
    enqueue_watch_for_repo(
        store,
        event_hub,
        WatchRepoTarget::LocalPath(repo_path),
        branch,
        from_sha,
        to_sha,
    )
    .await
}

async fn enqueue_watch_for_github_repo(
    store: &SharedStore,
    event_hub: &EventHub,
    repo_full_name: &str,
    git_ref: Option<&str>,
    from_sha: Option<&str>,
    to_sha: Option<&str>,
) -> u32 {
    enqueue_watch_for_repo(
        store,
        event_hub,
        WatchRepoTarget::GitHubRepo(repo_full_name),
        git_ref,
        from_sha,
        to_sha,
    )
    .await
}

async fn enqueue_watch_for_repo(
    store: &SharedStore,
    event_hub: &EventHub,
    repo_target: WatchRepoTarget<'_>,
    git_ref: Option<&str>,
    from_sha: Option<&str>,
    to_sha: Option<&str>,
) -> u32 {
    if !is_main_ref(git_ref) {
        return 0;
    }

    let triggers = match store.list_triggers_by_signal(Signal::Repo.as_i32()).await {
        Ok(triggers) => triggers,
        Err(err) => {
            tracing::warn!(
                error = %err,
                source = repo_target.label(),
                "failed to list repo triggers"
            );
            return 0;
        }
    };

    let reason = build_push_reason(git_ref, from_sha, to_sha);
    let from_sha = from_sha.unwrap_or("");
    let to_sha = to_sha.unwrap_or("");

    let mut matched = 0_u32;
    for mut trigger in triggers {
        if !trigger.enabled {
            continue;
        }

        let wave = match store.get_wave(&trigger.wave_id).await {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::warn!(
                    trigger_id = %trigger.id,
                    source = repo_target.label(),
                    error = %err,
                    "failed loading wave for repo trigger"
                );
                continue;
            }
        };
        if wave.status() == WaveStatus::Paused || !wave_matches_watch_repo(&wave, repo_target) {
            continue;
        }

        update_watch_main_sha(store, &mut trigger, to_sha).await;

        let outcome = enqueue_pending_activation(
            store,
            event_hub,
            ActivationEnvelope::new(
                &trigger.wave_id,
                Some(&trigger.id),
                reason.clone(),
                from_sha,
                to_sha,
                "main",
            ),
        )
        .await;
        if outcome.is_some() {
            matched += 1;
        }
    }

    matched
}

fn wave_matches_watch_repo(wave: &Wave, repo_target: WatchRepoTarget<'_>) -> bool {
    match repo_target {
        WatchRepoTarget::LocalPath(repo_path) => canonicalize_existing_path(Path::new(wave.repo()))
            .ok()
            .is_some_and(|path| path.to_string_lossy() == repo_path),
        WatchRepoTarget::GitHubRepo(repo_full_name) => {
            github_repo_from_local(Path::new(wave.repo()))
                .as_deref()
                .is_some_and(|wave_repo_full_name| wave_repo_full_name == repo_full_name)
        }
    }
}

async fn update_watch_main_sha(store: &SharedStore, trigger: &mut Trigger, to_sha: &str) {
    if to_sha.is_empty() {
        return;
    }

    trigger.last_main_sha = Some(to_sha.to_string());
    let _ = store.update_trigger(trigger).await;
}

fn is_main_ref(value: Option<&str>) -> bool {
    value.is_none_or(|ref_name| {
        ref_name == "main" || ref_name == "refs/heads/main" || ref_name.ends_with("/main")
    })
}

fn build_push_reason(branch: Option<&str>, from_sha: Option<&str>, to_sha: Option<&str>) -> String {
    let branch = branch.unwrap_or("main");
    let from_sha = from_sha.unwrap_or("");
    let to_sha = to_sha.unwrap_or("");
    if from_sha.is_empty() || to_sha.is_empty() {
        return format!("{branch} updated");
    }
    format!("{branch} advanced {from_sha}..{to_sha}")
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
        .to_ascii_lowercase();

    match event_kind.as_str() {
        "push" => {
            let event = serde_json::from_slice::<GitHubPushEvent>(&body).map_err(|err| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    ApiMessage::Untrusted(err.to_string()),
                )
            })?;

            let matched = enqueue_watch_for_github_repo(
                &state.store,
                &state.event_hub,
                &event.repository.full_name,
                Some(&event.git_ref),
                Some(&event.before),
                Some(&event.after),
            )
            .await;

            Ok(Json(serde_json::json!({ "ok": true, "matched": matched })))
        }
        "check_run" => {
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

            Ok(Json(serde_json::json!({ "ok": true, "matched": matched })))
        }
        "pull_request" => {
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
                let handled = crate::lfd::queue::handle_pr_merged_with_events(
                    &state.store,
                    &state.github,
                    &wave_id,
                    event.pull_request.number,
                    merged_at,
                    Some(&state.event_hub),
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
            Ok(Json(
                serde_json::json!({ "ok": true, "processed": processed }),
            ))
        }
        _ => Ok(Json(
            serde_json::json!({ "ok": true, "matched": 0, "skipped": true }),
        )),
    }
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
            .any(|run| run.pr.and_then(|pr| pr.number) == Some(pr_number));
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
    if run.snapshot.flow == CI_FIX_FLOW {
        return false;
    }

    let Some(pr) = run.pr.as_ref() else {
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
    let pr = run.pr.as_ref()?;
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
    use crate::lfd::types::{
        PullRequest, Signal, Trigger, Wave, WaveMode, WaveRunSnapshot, WaveRunStatus, WaveStatus,
    };
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
            provider_auth: ProviderAuthService::new(store.clone()),
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
                from_sha: None,
                to_sha: None,
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
                from_sha: None,
                to_sha: None,
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

    #[tokio::test]
    async fn github_push_enqueues_watch_activation() {
        let state = test_http_state().await;
        let repo_dir = tempdir().expect("repo tempdir");
        std::process::Command::new("git")
            .args(["-C", repo_dir.path().to_string_lossy().as_ref(), "init"])
            .status()
            .expect("git init should run");
        std::process::Command::new("git")
            .args([
                "-C",
                repo_dir.path().to_string_lossy().as_ref(),
                "remote",
                "add",
                "origin",
                "git@github.com:loopflowstudio/loopflow.git",
            ])
            .status()
            .expect("git remote add should run");

        let wave = Wave {
            id: LfdId::new(),
            name: "watch-wave".to_string(),
            repo: repo_dir.path().to_string_lossy().to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: vec![],
            area: vec![],
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
        };
        state.store.create_wave(&wave).await.expect("create wave");
        let trigger = Trigger {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            source_wave_id: None,
            signal: Signal::Repo,
            flow: None,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
            max_iterations: None,
        };
        state
            .store
            .create_trigger(&trigger)
            .await
            .expect("create repo trigger");

        let matched = enqueue_watch_for_github_repo(
            &state.store,
            &state.event_hub,
            "loopflowstudio/loopflow",
            Some("refs/heads/main"),
            Some("abc"),
            Some("def"),
        )
        .await;
        assert_eq!(matched, 1);

        let pending = state
            .store
            .list_pending_activations(&wave.id)
            .await
            .expect("pending activations");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].to_sha, "def");
    }

    fn wave_run_with_pr(flow: &str, pr_state: Option<&str>, branch: Option<&str>) -> WaveRun {
        WaveRun {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            snapshot: WaveRunSnapshot {
                repo: ".".to_string(),
                flow: flow.to_string(),
                direction: Vec::new(),
                area: Vec::new(),
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
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: "wave-group".to_string(),
            stack_status: crate::lfd::types::WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: Some(PullRequest {
                url: "https://example.test/pr/1".to_string(),
                number: Some(1),
                state: pr_state.map(ToString::to_string),
                title: Some("test".to_string()),
                branch: branch.map(ToString::to_string),
            }),
        }
    }

    #[test]
    fn run_matches_ci_target_only_matches_open_main_prs() {
        let run = wave_run_with_pr("build", Some("open"), Some("feature"));
        assert!(run_matches_ci_target(&run, Some("feature"), Some(1)));

        let closed = wave_run_with_pr("build", Some("closed"), Some("feature"));
        assert!(!run_matches_ci_target(&closed, Some("feature"), Some(1)));

        let unknown_state = wave_run_with_pr("build", None, Some("feature"));
        assert!(!run_matches_ci_target(
            &unknown_state,
            Some("feature"),
            Some(1)
        ));

        let ci_fix = wave_run_with_pr("ci-fix", Some("open"), Some("feature"));
        assert!(!run_matches_ci_target(&ci_fix, Some("feature"), Some(1)));
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
