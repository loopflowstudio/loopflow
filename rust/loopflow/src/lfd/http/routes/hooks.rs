//! GitHub webhook ingress — the gatekeeper's ears, translating inward to `lf`.
//!
//! Webhooks no longer feed the trigger/activation machinery. Each surviving
//! event execs the `lf` surface (collapse call #1). In M0, CI failures and
//! main pushes still use attributed `lf chat` as a compatibility notification
//! path so the demo keeps working; M1 should move coordination to durable
//! facts plus explicit `lf` commands where that is the real contract.
//!
//! - **check_run failure** → `lf chat --wave <wave> "CI failed: …"` — the
//!   wave's mind decides whether and how to dispatch a fix.
//! - **PR merged** → `lf op queue reconcile --wave <wave>` — the queue verb
//!   owns stack-status inference and promotion.
//! - **push to main** → `lf chat --wave <wave> "main moved: …"` for every
//!   wave in the repo — the mind decides to rebase/integrate with judgment.
//!
//! Execs are spawned detached; a wave whose server is down bounces the chat
//! with exit ≠ 0 — logged at warn and, for CI failures, the dedupe key is NOT
//! recorded, so the next delivery of that wave+sha replays instead of
//! vanishing. No wave resolved → log-and-drop.

// TODO(M1/M3): preserve these ingress reliability mechanisms under the
// gatekeeper/argv owner: signature verification, plan-then-exec tests,
// bounced-chat replay, and CI dedupe only after delivery succeeds.
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use bytes::Bytes;
use tokio::sync::Mutex;

use crate::lfd::executor::resolve_lf_binary;
use crate::lfd::github::{
    github_repo_from_local, verify_webhook_signature, GitHubCheckRunEvent, GitHubPullRequestEvent,
    GitHubPushEvent,
};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::types::{Run, Wave, CI_FIX_FLOW};
use crate::lfdb::SharedStore;

#[derive(Debug, Clone)]
struct WaveCiTarget {
    wave_id: LfdId,
    pr_number: u32,
}

// -- Planned lf execs ------------------------------------------------------

/// One `lf` invocation the gatekeeper will spawn — argv after the binary.
/// Planners return these so tests assert on the exact command line without
/// spawning anything. `dedupe_key` (CI failures: `<wave_id>:<sha>`) is
/// recorded in the shared cache only after the exec exits 0 — a bounced chat
/// leaves the key absent so the wave+sha can replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfExec {
    pub args: Vec<String>,
    pub dedupe_key: Option<String>,
}

impl LfExec {
    fn chat(wave: &str, text: String, from: &str) -> Self {
        Self {
            args: vec![
                "chat".to_string(),
                "--wave".to_string(),
                wave.to_string(),
                "--from".to_string(),
                from.to_string(),
                text,
            ],
            dedupe_key: None,
        }
    }

    fn queue_reconcile(wave: &str) -> Self {
        Self {
            args: vec![
                "op".to_string(),
                "queue".to_string(),
                "reconcile".to_string(),
                "--wave".to_string(),
                wave.to_string(),
            ],
            dedupe_key: None,
        }
    }

    fn with_dedupe_key(mut self, key: String) -> Self {
        self.dedupe_key = Some(key);
        self
    }
}

/// Spawn each exec detached — the webhook response never waits on `lf`.
fn spawn_lf_execs(cache: &Arc<Mutex<HashSet<String>>>, execs: Vec<LfExec>) {
    let lf = resolve_lf_binary();
    for exec in execs {
        let lf = lf.clone();
        let cache = cache.clone();
        tokio::spawn(async move {
            settle_exec(&lf, exec, &cache).await;
        });
    }
}

/// Run one exec to completion and settle its dedupe key: exit 0 records the
/// key (that wave+sha has been heard); a bounce (wave server down, exit ≠ 0)
/// or spawn failure records nothing, so the next webhook for the same
/// wave+sha replays instead of being swallowed.
async fn settle_exec(lf: &std::path::Path, exec: LfExec, cache: &Arc<Mutex<HashSet<String>>>) {
    let result = tokio::process::Command::new(lf)
        .args(&exec.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    match result {
        Ok(status) if status.success() => {
            if let Some(key) = exec.dedupe_key {
                cache.lock().await.insert(key);
            }
        }
        Ok(status) => tracing::warn!(
            args = ?exec.args,
            dedupe_key = ?exec.dedupe_key,
            code = ?status.code(),
            "lf exec bounced (no live subscriber); will replay on next delivery"
        ),
        Err(err) => tracing::warn!(
            args = ?exec.args,
            dedupe_key = ?exec.dedupe_key,
            error = %err,
            "lf exec failed to spawn"
        ),
    }
}

/// Push-to-main → an attributed notification for every wave in the pushed repo.
/// Non-main refs plan nothing.
async fn plan_push_notifications(
    store: &SharedStore,
    repo_full_name: &str,
    git_ref: &str,
    before: &str,
    after: &str,
) -> Result<Vec<LfExec>, String> {
    if !is_main_ref(Some(git_ref)) {
        return Ok(Vec::new());
    }
    let waves = store
        .list_waves(None)
        .await
        .map_err(|err| err.to_string())?;
    let text = main_moved_text(before, after);
    Ok(waves
        .iter()
        .filter(|wave| wave_in_github_repo(wave, repo_full_name))
        .map(|wave| LfExec::chat(wave.name(), text.clone(), "github"))
        .collect())
}

fn main_moved_text(before: &str, after: &str) -> String {
    match (before.is_empty(), after.is_empty()) {
        (false, false) => format!("main moved: {before}..{after}"),
        (true, false) => format!("main moved: {after}"),
        _ => "main moved".to_string(),
    }
}

/// Failed check_run → an attributed notification to each wave owning an open
/// PR the check ran against. Deduped per wave+commit through the shared
/// CI-failure cache so a red matrix reports once — but the key is only RECORDED once the spawned
/// exec exits 0 (see [`settle_exec`]); planning just reads the cache, so a
/// bounced chat replays. No wave resolved → empty (the caller drops).
async fn plan_check_run_notifications(
    store: &SharedStore,
    cache: &Arc<Mutex<HashSet<String>>>,
    event: &GitHubCheckRunEvent,
) -> Result<Vec<LfExec>, String> {
    let mut execs = Vec::new();
    let mut planned: HashSet<String> = HashSet::new();
    for pr in &event.check_run.pull_requests {
        let targets = find_wave_ci_targets(
            store,
            &event.repository.full_name,
            &pr.head.branch,
            Some(pr.number),
        )
        .await?;
        for target in targets {
            let key = format!("{}:{}", target.wave_id, event.check_run.head_sha);
            if cache.lock().await.contains(&key) || !planned.insert(key.clone()) {
                continue;
            }
            let Some(wave) = store
                .get_wave(&target.wave_id)
                .await
                .map_err(|err| err.to_string())?
            else {
                continue;
            };
            execs.push(
                LfExec::chat(
                    wave.name(),
                    format!(
                        "CI failed: {} on PR #{} — {}",
                        event.check_run.name, target.pr_number, event.check_run.html_url
                    ),
                    "ci",
                )
                .with_dedupe_key(key),
            );
        }
    }
    Ok(execs)
}

/// PR merged → `lf op queue reconcile` for each wave holding that PR in its
/// stack. Replaces the in-process `handle_pr_merged_with_events` call.
async fn plan_pr_merged_reconciles(
    store: &SharedStore,
    repo_full_name: &str,
    pr_number: u32,
) -> Result<Vec<LfExec>, String> {
    let wave_ids = find_waves_for_pr(store, repo_full_name, pr_number).await?;
    let mut execs = Vec::new();
    for wave_id in wave_ids {
        let Some(wave) = store
            .get_wave(&wave_id)
            .await
            .map_err(|err| err.to_string())?
        else {
            continue;
        };
        execs.push(LfExec::queue_reconcile(wave.name()));
    }
    Ok(execs)
}

fn wave_in_github_repo(wave: &Wave, repo_full_name: &str) -> bool {
    github_repo_from_local(Path::new(wave.repo()))
        .as_deref()
        .is_some_and(|wave_repo| wave_repo == repo_full_name)
}

fn is_main_ref(value: Option<&str>) -> bool {
    value.is_none_or(|ref_name| {
        ref_name == "main" || ref_name == "refs/heads/main" || ref_name.ends_with("/main")
    })
}

// -- The webhook handler ---------------------------------------------------

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

            let execs = plan_push_notifications(
                &state.store,
                &event.repository.full_name,
                &event.git_ref,
                &event.before,
                &event.after,
            )
            .await
            .map_err(|err| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ApiMessage::Untrusted(err),
                )
            })?;
            let matched = execs.len() as u32;
            if matched == 0 {
                tracing::debug!(
                    repo = %event.repository.full_name,
                    git_ref = %event.git_ref,
                    "push webhook matched no waves; dropped"
                );
            }
            spawn_lf_execs(&state.ci_failure_cache, execs);
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

            let execs = plan_check_run_notifications(&state.store, &state.ci_failure_cache, &event)
                .await
                .map_err(|err| {
                    api_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ApiMessage::Untrusted(err),
                    )
                })?;
            let matched = execs.len() as u32;
            if matched == 0 {
                tracing::debug!(
                    repo = %event.repository.full_name,
                    check_name = %event.check_run.name,
                    "CI failure webhook resolved no wave; dropped"
                );
            } else {
                tracing::info!(
                    repo = %event.repository.full_name,
                    commit_sha = %event.check_run.head_sha,
                    check_name = %event.check_run.name,
                    matched,
                    "CI failure notification delivered to waves via lf chat"
                );
            }
            spawn_lf_execs(&state.ci_failure_cache, execs);
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
            let execs = plan_pr_merged_reconciles(
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
            let processed = execs.len() as u32;
            spawn_lf_execs(&state.ci_failure_cache, execs);
            Ok(Json(
                serde_json::json!({ "ok": true, "processed": processed }),
            ))
        }
        _ => Ok(Json(
            serde_json::json!({ "ok": true, "matched": 0, "skipped": true }),
        )),
    }
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
        if !wave_in_github_repo(&wave, repo_full_name) {
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

        let Some(target) = find_wave_ci_target(store, &wave, branch, pr_number).await? else {
            continue;
        };
        targets.push(target);
    }

    Ok(targets)
}

async fn find_wave_ci_target(
    store: &SharedStore,
    wave: &Wave,
    branch: Option<&str>,
    pr_number: Option<u32>,
) -> Result<Option<WaveCiTarget>, String> {
    let runs = store
        .list_runs(Some(wave.id()), None)
        .await
        .map_err(|err| err.to_string())?;
    let run = runs
        .into_iter()
        .find(|run| run_matches_ci_target(run, branch, pr_number));

    Ok(run.and_then(|run| wave_ci_target(wave.id(), &run)))
}

fn run_matches_ci_target(run: &Run, branch: Option<&str>, pr_number: Option<u32>) -> bool {
    if run.flow == CI_FIX_FLOW {
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

fn wave_ci_target(wave_id: &LfdId, run: &Run) -> Option<WaveCiTarget> {
    let pr = run.pr.as_ref()?;
    Some(WaveCiTarget {
        wave_id: wave_id.clone(),
        pr_number: pr.number?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::github::{CheckRun, CheckRunPR, CheckRunRef, GitHubRepository};
    use crate::lfd::types::{PullRequest, RepoWork, RunStackStatus, RunStatus, WaveStatus};
    use crate::lfdb::{open_store, StorageConfig};
    use std::sync::Arc;
    use tempfile::tempdir;
    use time::OffsetDateTime;
    use tokio::sync::Mutex;

    async fn temp_store(dir: &std::path::Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(dir.join("lfd.db")))
                .await
                .expect("open sqlite store"),
        )
    }

    /// A local git repo with `origin` pointing at the given GitHub repo, so
    /// `github_repo_from_local` resolves.
    fn github_backed_repo(dir: &std::path::Path, full_name: &str) -> std::path::PathBuf {
        let repo = dir.join(full_name.replace('/', "-"));
        std::fs::create_dir_all(&repo).expect("repo dir");
        let repo_str = repo.to_string_lossy().to_string();
        std::process::Command::new("git")
            .args(["-C", &repo_str, "init"])
            .status()
            .expect("git init");
        std::process::Command::new("git")
            .args([
                "-C",
                &repo_str,
                "remote",
                "add",
                "origin",
                &format!("git@github.com:{full_name}.git"),
            ])
            .status()
            .expect("git remote add");
        repo
    }

    fn make_wave(name: &str, repo: &std::path::Path) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repos: vec![RepoWork {
                repo: repo.to_string_lossy().to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn run_with_open_pr(wave: &Wave, flow: &str, branch: &str, pr_number: u32) -> Run {
        Run {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            repo: wave.repo().to_string(),
            flow: flow.to_string(),
            task: None,
            direction: Vec::new(),
            area: Vec::new(),
            iteration: 0,
            step_index: 0,
            status: RunStatus::Running,
            worktree: "/tmp/worktree".to_string(),
            branch: branch.to_string(),
            started_at: None,
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id().to_string(),
            stack_status: RunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: Some(PullRequest {
                url: format!("https://example.test/pr/{pr_number}"),
                number: Some(pr_number),
                state: Some("open".to_string()),
                title: Some("test".to_string()),
                branch: Some(branch.to_string()),
            }),
        }
    }

    fn check_run_event(full_name: &str, branch: &str, pr_number: u32) -> GitHubCheckRunEvent {
        GitHubCheckRunEvent {
            action: "completed".to_string(),
            check_run: CheckRun {
                id: 9,
                name: "test-check".to_string(),
                head_sha: "abc123".to_string(),
                status: "completed".to_string(),
                conclusion: Some("failure".to_string()),
                pull_requests: vec![CheckRunPR {
                    number: pr_number,
                    head: CheckRunRef {
                        branch: branch.to_string(),
                        sha: "abc123".to_string(),
                    },
                }],
                html_url: "https://example.test/logs".to_string(),
            },
            repository: GitHubRepository {
                full_name: full_name.to_string(),
            },
        }
    }

    /// Push to main notifies every wave in the repo — and only those.
    #[tokio::test]
    async fn push_to_main_plans_chat_for_each_wave_in_repo() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        let other_repo = github_backed_repo(tmp.path(), "loopflowstudio/other");
        store
            .create_wave(&make_wave("ship", &repo))
            .await
            .expect("wave ship");
        store
            .create_wave(&make_wave("systems", &repo))
            .await
            .expect("wave systems");
        store
            .create_wave(&make_wave("elsewhere", &other_repo))
            .await
            .expect("wave elsewhere");

        let execs = plan_push_notifications(
            &store,
            "loopflowstudio/loopflow",
            "refs/heads/main",
            "abc",
            "def",
        )
        .await
        .expect("plan");

        let mut argvs: Vec<Vec<String>> = execs.into_iter().map(|exec| exec.args).collect();
        argvs.sort();
        assert_eq!(
            argvs,
            vec![
                vec![
                    "chat".to_string(),
                    "--wave".to_string(),
                    "ship".to_string(),
                    "--from".to_string(),
                    "github".to_string(),
                    "main moved: abc..def".to_string(),
                ],
                vec![
                    "chat".to_string(),
                    "--wave".to_string(),
                    "systems".to_string(),
                    "--from".to_string(),
                    "github".to_string(),
                    "main moved: abc..def".to_string(),
                ],
            ]
        );
    }

    #[tokio::test]
    async fn push_to_feature_branch_plans_nothing() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        store
            .create_wave(&make_wave("ship", &repo))
            .await
            .expect("wave");

        let execs = plan_push_notifications(
            &store,
            "loopflowstudio/loopflow",
            "refs/heads/feature",
            "abc",
            "def",
        )
        .await
        .expect("plan");
        assert!(execs.is_empty());
    }

    /// CI failure resolves the owning wave and plans the chat exec carrying
    /// the wave+sha dedupe key; an unknown repo drops.
    #[tokio::test]
    async fn check_run_failure_plans_attributed_chat_with_dedupe_key() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        let wave = make_wave("ship", &repo);
        store.create_wave(&wave).await.expect("wave");
        store
            .create_run(&run_with_open_pr(&wave, "build", "feature", 1))
            .await
            .expect("run");

        let cache = Arc::new(Mutex::new(HashSet::new()));
        let event = check_run_event("loopflowstudio/loopflow", "feature", 1);

        let execs = plan_check_run_notifications(&store, &cache, &event)
            .await
            .expect("plan");
        let expected_key = format!("{}:abc123", wave.id());
        assert_eq!(
            execs,
            vec![LfExec::chat(
                "ship",
                "CI failed: test-check on PR #1 — https://example.test/logs".to_string(),
                "ci",
            )
            .with_dedupe_key(expected_key.clone())]
        );

        // Planning must NOT record the key — only a delivered exec does.
        assert!(!cache.lock().await.contains(&expected_key));

        // Unknown repo: no wave resolves, dropped.
        let unknown = check_run_event("loopflowstudio/ghost", "feature", 1);
        let dropped = plan_check_run_notifications(&store, &cache, &unknown)
            .await
            .expect("plan unknown");
        assert!(dropped.is_empty(), "no wave resolved → drop");
    }

    /// The bounce contract: a failed exec leaves the dedupe key absent so
    /// the same wave+sha replays; a delivered exec records it so the replay
    /// dedupes to nothing.
    #[tokio::test]
    async fn bounced_ci_exec_leaves_key_absent_so_replay_succeeds() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        let wave = make_wave("ship", &repo);
        store.create_wave(&wave).await.expect("wave");
        store
            .create_run(&run_with_open_pr(&wave, "build", "feature", 1))
            .await
            .expect("run");

        let cache = Arc::new(Mutex::new(HashSet::new()));
        let event = check_run_event("loopflowstudio/loopflow", "feature", 1);

        // First delivery bounces (wave server down → exit ≠ 0).
        let execs = plan_check_run_notifications(&store, &cache, &event)
            .await
            .expect("plan");
        assert_eq!(execs.len(), 1);
        settle_exec(
            std::path::Path::new("/usr/bin/false"),
            execs[0].clone(),
            &cache,
        )
        .await;
        assert!(cache.lock().await.is_empty(), "bounce records nothing");

        // Replay: the same wave+sha plans again…
        let replay = plan_check_run_notifications(&store, &cache, &event)
            .await
            .expect("plan replay");
        assert_eq!(replay.len(), 1, "bounced wave+sha replays");

        // …and this time delivery succeeds, so the key settles.
        settle_exec(
            std::path::Path::new("/usr/bin/true"),
            replay[0].clone(),
            &cache,
        )
        .await;
        let deduped = plan_check_run_notifications(&store, &cache, &event)
            .await
            .expect("plan deduped");
        assert!(deduped.is_empty(), "delivered wave+sha never reports twice");
    }

    /// PR merged plans the queue-reconcile verb for the owning wave.
    #[tokio::test]
    async fn pr_merged_plans_queue_reconcile_for_owning_wave() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        let wave = make_wave("ship", &repo);
        store.create_wave(&wave).await.expect("wave");
        store
            .create_run(&run_with_open_pr(&wave, "build", "feature", 7))
            .await
            .expect("run");

        let execs = plan_pr_merged_reconciles(&store, "loopflowstudio/loopflow", 7)
            .await
            .expect("plan");
        assert_eq!(execs, vec![LfExec::queue_reconcile("ship")]);

        // A PR nobody holds plans nothing.
        let none = plan_pr_merged_reconciles(&store, "loopflowstudio/loopflow", 99)
            .await
            .expect("plan none");
        assert!(none.is_empty());
    }

    fn run_with_pr(flow: &str, pr_state: Option<&str>, branch: Option<&str>) -> Run {
        Run {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            repo: ".".to_string(),
            flow: flow.to_string(),
            task: None,
            direction: Vec::new(),
            area: Vec::new(),
            iteration: 0,
            step_index: 0,
            status: RunStatus::Running,
            worktree: "/tmp/worktree".to_string(),
            branch: "feature".to_string(),
            started_at: None,
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            execution_cursor: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: "wave-group".to_string(),
            stack_status: crate::lfd::types::RunStackStatus::Active,
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
        let run = run_with_pr("build", Some("open"), Some("feature"));
        assert!(run_matches_ci_target(&run, Some("feature"), Some(1)));

        let closed = run_with_pr("build", Some("closed"), Some("feature"));
        assert!(!run_matches_ci_target(&closed, Some("feature"), Some(1)));

        let unknown_state = run_with_pr("build", None, Some("feature"));
        assert!(!run_matches_ci_target(
            &unknown_state,
            Some("feature"),
            Some(1)
        ));

        let ci_fix = run_with_pr("ci-fix", Some("open"), Some("feature"));
        assert!(!run_matches_ci_target(&ci_fix, Some("feature"), Some(1)));
    }
}
