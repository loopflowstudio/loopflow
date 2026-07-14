//! GitHub webhook ingress — the gatekeeper's ears, translating inward.
//!
//! Webhooks no longer feed the trigger/activation machinery. A webhook fact
//! is machine speech, so it rides the bus (`lf radio pub --from github`) and
//! survives a sleeping wave. Durable Task Sessions own delivery state.
//!
//! - **check_run failure** → `lf radio pub --channel <wave> --from github
//!   "CI failed: …"` — the wave's loop decides how to respond.
//! - **PR merged** → complete the Task Session that owns that PR.
//! - **push to main** → the same, `"main moved: …"`, for every wave in the
//!   repo — the loop decides to rebase/integrate with judgment.
//!
//! Radio commands run asynchronously. Publishing is a store INSERT, so the only
//! bounce left is a machine with no registry db (exit 0, dropped with a
//! note); the CI dedupe key is recorded only after the command exits 0, so a
//! spawn failure replays on the next delivery. No wave resolved →
//! log-and-drop.

// TODO(M1/M3): preserve these ingress reliability mechanisms under the
// gatekeeper/argv owner: signature verification, plan-then-run tests,
// failed-publish replay, and CI dedupe only after delivery succeeds.
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use bytes::Bytes;
use tokio::sync::Mutex;

use crate::engine::git::{
    delete_local_branch, get_default_branch, is_clean_ignoring_scratch, worktree_remove,
};
use crate::engine::process::resolve_lf_binary;
use crate::engine::worktrees::{list_porcelain, main_repo_root};
use crate::lfd::github::{
    github_repo_from_local, verify_webhook_signature, GitHubCheckRunEvent, GitHubDeleteEvent,
    GitHubPullRequestEvent, GitHubPushEvent,
};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::types::Wave;
use crate::lfdb::SharedStore;
use crate::task::{
    PmWritebackOperation, PmWritebackState, TaskEventKind, TaskSession, TaskSessionStatus,
};

#[derive(Debug, Clone)]
struct WaveCiTarget {
    wave_id: LfdId,
    pr_number: u32,
}

// -- Planned radio commands ------------------------------------------------

/// One `lf` invocation the gatekeeper will spawn — argv after the binary.
/// Planners return these so tests assert on the exact command line without
/// spawning anything. `dedupe_key` (CI failures: `<wave_id>:<sha>`) is
/// recorded in the shared cache only after the command exits 0 — a failed command
/// leaves the key absent so the wave+sha can replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadioCommand {
    pub args: Vec<String>,
    pub dedupe_key: Option<String>,
}

impl RadioCommand {
    /// A webhook fact is machine speech: it rides the bus with a byline, so
    /// it survives a sleeping wave and folds into the thread attributed when
    /// the listener next sweeps (`wave::bus`). Chat is the human's verb.
    fn radio(wave: &str, text: String, from: &str) -> Self {
        Self {
            args: vec![
                "radio".to_string(),
                "pub".to_string(),
                "--channel".to_string(),
                wave.to_string(),
                "--from".to_string(),
                from.to_string(),
                text,
            ],
            dedupe_key: None,
        }
    }

    fn with_dedupe_key(mut self, key: String) -> Self {
        self.dedupe_key = Some(key);
        self
    }
}

/// Start each command asynchronously — the webhook response never waits on `lf`.
fn spawn_radio_commands(cache: &Arc<Mutex<HashSet<String>>>, commands: Vec<RadioCommand>) {
    let lf = resolve_lf_binary();
    for command in commands {
        let lf = lf.clone();
        let cache = cache.clone();
        tokio::spawn(async move {
            settle_radio_command(&lf, command, &cache).await;
        });
    }
}

/// Run one command to completion and settle its dedupe key: exit 0 records the
/// key (the bus accepted the frame); a nonzero exit or start failure records
/// nothing, so the next webhook for the same wave+sha replays.
async fn settle_radio_command(
    lf: &std::path::Path,
    command: RadioCommand,
    cache: &Arc<Mutex<HashSet<String>>>,
) {
    let result = tokio::process::Command::new(lf)
        .args(&command.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await;
    match result {
        Ok(status) if status.success() => {
            if let Some(key) = command.dedupe_key {
                cache.lock().await.insert(key);
            }
        }
        Ok(status) => tracing::warn!(
            args = ?command.args,
            dedupe_key = ?command.dedupe_key,
            code = ?status.code(),
            "lf radio publish failed; will replay on next delivery"
        ),
        Err(err) => tracing::warn!(
            args = ?command.args,
            dedupe_key = ?command.dedupe_key,
            error = %err,
            "lf radio command failed to start"
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
) -> Result<Vec<RadioCommand>, String> {
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
        .map(|wave| RadioCommand::radio(wave.name(), text.clone(), "github"))
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
/// command exits 0 (see [`settle_radio_command`]); planning just reads the cache, so a
/// failed publish replays. No wave resolved → empty (the caller drops).
async fn plan_check_run_notifications(
    store: &SharedStore,
    cache: &Arc<Mutex<HashSet<String>>>,
    event: &GitHubCheckRunEvent,
) -> Result<Vec<RadioCommand>, String> {
    let mut commands = Vec::new();
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
            commands.push(
                RadioCommand::radio(
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
    Ok(commands)
}

/// PR merged → complete each durable Task Session that owns that PR.
async fn complete_merged_task_sessions(
    store: &SharedStore,
    repo_full_name: &str,
    pr_number: u32,
) -> Result<u32, String> {
    let mut processed = 0;
    for mut session in store
        .list_task_sessions(None)
        .await
        .map_err(|error| error.to_string())?
    {
        let Some(pull_request) = session
            .pull_request
            .clone()
            .filter(|pull_request| pull_request.number == pr_number)
        else {
            continue;
        };
        let Some(wave) = store
            .get_wave(&session.wave_id)
            .await
            .map_err(|err| err.to_string())?
        else {
            continue;
        };
        if !wave_in_github_repo(&wave, repo_full_name)
            || matches!(
                session.status,
                TaskSessionStatus::Merged | TaskSessionStatus::Abandoned
            )
        {
            continue;
        }
        let from = session.status;
        session.set_status(
            TaskSessionStatus::Merged,
            format!("pull request #{pr_number} merged"),
        );
        session.pm_writeback = match crate::ops::task_pm::complete_task(
            Path::new(wave.repo()),
            &session.wave,
            session.issue.id.as_str(),
            &pull_request.url,
        )
        .await
        {
            Ok(()) => PmWritebackState::Current,
            Err(error) => PmWritebackState::Pending {
                operation: PmWritebackOperation::CompleteTask,
                error: error.to_string(),
            },
        };
        store
            .update_task_session(&session)
            .await
            .map_err(|error| error.to_string())?;
        store
            .append_task_event(
                &session.id,
                &TaskEventKind::StatusChanged {
                    from,
                    to: TaskSessionStatus::Merged,
                    reason: session.status_reason.clone(),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        store
            .append_task_event(
                &session.id,
                &TaskEventKind::Completed {
                    pull_request,
                    summary: session.status_reason.clone(),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        crate::lf::commands::chat::post_to_wave(
            &wave,
            &format!(
                "Task {} → merged: {}",
                session.issue.identifier, session.status_reason
            ),
        )
        .await
        .map_err(|error| error.to_string())?;
        processed += 1;
    }
    Ok(processed)
}

/// A deleted branch → remove the local worktree that was on it. GitHub deletes
/// the head branch on merge (when the repo setting is on), so a merged PR's
/// worktree self-cleans. This is a direct local git operation, not an `lf` command —
/// worktree lifecycle is lfd-owned infrastructure, like the resident's bootstrap.
///
/// Skips the default branch, the main checkout, and any tree with uncommitted
/// changes (scratch aside) — a `--force` removal is left to the human.
async fn remove_worktrees_for_deleted_branch(
    store: &SharedStore,
    repo_full_name: &str,
    branch: &str,
) -> Result<Vec<String>, String> {
    let waves = store
        .list_waves(None)
        .await
        .map_err(|err| err.to_string())?;
    let mut removed = Vec::new();
    let mut seen_repos: HashSet<String> = HashSet::new();
    for wave in waves {
        if !wave_in_github_repo(&wave, repo_full_name)
            || !seen_repos.insert(wave.repo().to_string())
        {
            continue;
        }
        match remove_branch_worktree(Path::new(wave.repo()), branch) {
            Ok(Some(path)) => removed.push(path),
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(repo = wave.repo(), branch, error = %err, "worktree cleanup failed")
            }
        }
    }
    Ok(removed)
}

fn remove_branch_worktree(repo: &Path, branch: &str) -> Result<Option<String>, String> {
    let main = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let default_branch = get_default_branch(&main).map_err(|err| err.to_string())?;
    if branch == default_branch {
        return Ok(None);
    }
    let found = list_porcelain(&main)
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|(_, wt_branch)| wt_branch.as_deref() == Some(branch));
    let Some((path, _)) = found else {
        return Ok(None);
    };
    if !is_clean_ignoring_scratch(&path).unwrap_or(false) {
        tracing::info!(
            ?path,
            branch,
            "worktree has uncommitted changes; left for manual cleanup"
        );
        return Ok(None);
    }
    worktree_remove(&main, &path).map_err(|err| err.to_string())?;
    let _ = delete_local_branch(&main, branch);
    Ok(Some(path.display().to_string()))
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

            let commands = plan_push_notifications(
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
            let matched = commands.len() as u32;
            if matched == 0 {
                tracing::debug!(
                    repo = %event.repository.full_name,
                    git_ref = %event.git_ref,
                    "push webhook matched no waves; dropped"
                );
            }
            spawn_radio_commands(&state.ci_failure_cache, commands);
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

            let commands =
                plan_check_run_notifications(&state.store, &state.ci_failure_cache, &event)
                    .await
                    .map_err(|err| {
                        api_error(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            ApiMessage::Untrusted(err),
                        )
                    })?;
            let matched = commands.len() as u32;
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
            spawn_radio_commands(&state.ci_failure_cache, commands);
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
            let processed = complete_merged_task_sessions(
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
            Ok(Json(
                serde_json::json!({ "ok": true, "processed": processed }),
            ))
        }
        "delete" => {
            let event = serde_json::from_slice::<GitHubDeleteEvent>(&body).map_err(|err| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    ApiMessage::Untrusted(err.to_string()),
                )
            })?;
            if event.ref_type != "branch" {
                return Ok(Json(
                    serde_json::json!({ "ok": true, "removed": 0, "skipped": true }),
                ));
            }
            let removed = remove_worktrees_for_deleted_branch(
                &state.store,
                &event.repository.full_name,
                &event.git_ref,
            )
            .await
            .map_err(|err| {
                api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ApiMessage::Untrusted(err),
                )
            })?;
            if !removed.is_empty() {
                tracing::info!(
                    repo = %event.repository.full_name,
                    branch = %event.git_ref,
                    count = removed.len(),
                    "removed worktrees for deleted branch"
                );
            }
            Ok(Json(
                serde_json::json!({ "ok": true, "removed": removed.len() }),
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
    let tasks = store
        .list_task_sessions(Some(wave.id()))
        .await
        .map_err(|err| err.to_string())?;
    let task = tasks
        .into_iter()
        .find(|task| task_matches_ci_target(task, branch, pr_number));

    Ok(task.and_then(|task| wave_ci_target(wave.id(), &task)))
}

fn task_matches_ci_target(
    task: &TaskSession,
    branch: Option<&str>,
    pr_number: Option<u32>,
) -> bool {
    let Some(pr) = task.pull_request.as_ref() else {
        return false;
    };
    !task.status.is_terminal()
        && branch.is_none_or(|branch| task.branch == branch)
        && pr_number.is_none_or(|number| pr.number == number)
}

fn is_failed_check_run(status: &str, conclusion: Option<&str>) -> bool {
    status == "completed" && conclusion == Some("failure")
}

fn wave_ci_target(wave_id: &LfdId, task: &TaskSession) -> Option<WaveCiTarget> {
    let pr = task.pull_request.as_ref()?;
    Some(WaveCiTarget {
        wave_id: wave_id.clone(),
        pr_number: pr.number,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::github::{CheckRun, CheckRunPR, CheckRunRef, GitHubRepository};
    use crate::lfdb::{open_store, StorageConfig};
    use crate::task::{
        LinearIssueId, LinearIssueRef, LinearProjectId, LinearProjectRef, PmWritebackState,
        PullRequestRef, TaskSession, TaskSessionId,
    };
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
        Wave::new(
            LfdId::new(),
            name.to_string(),
            repo.to_string_lossy().to_string(),
        )
    }

    fn task_with_open_pr(wave: &Wave, pr_number: u32) -> TaskSession {
        let now = OffsetDateTime::now_utc();
        TaskSession {
            id: TaskSessionId::new(),
            issue: LinearIssueRef {
                id: LinearIssueId::new("issue-uuid").expect("issue id"),
                identifier: "INF-123".to_string(),
                title: "Ship task sessions".to_string(),
                description: String::new(),
            },
            project: LinearProjectRef {
                id: LinearProjectId::new("project-uuid").expect("project id"),
                slug: "delivery".to_string(),
                name: "Delivery".to_string(),
                context: "Definition:\nShip task sessions.".to_string(),
            },
            pm_snapshot_synced_at: now.unix_timestamp(),
            pm_snapshot_warning: None,
            pm_writeback: PmWritebackState::Current,
            wave_id: wave.id().clone(),
            wave: wave.name().to_string(),
            supervisor: crate::child_session::SessionSupervisor::Wave {
                wave_id: wave.id().clone(),
            },
            current_directive_version: 0,
            incorporated_directive_version: 0,
            status: TaskSessionStatus::Submitted,
            status_reason: format!("pull request #{pr_number} is open for review"),
            status_at: now,
            worktree: std::path::PathBuf::from("/tmp/task-inf-123"),
            branch: "jack/inf-123".to_string(),
            base_commit: "deadbeef".to_string(),
            agent: "codex".to_string(),
            provider: "codex".to_string(),
            provider_session_id: Some("thread-1".to_string()),
            process: None,
            pull_request: Some(PullRequestRef {
                number: pr_number,
                url: format!("https://example.test/pr/{pr_number}"),
            }),
            created_at: now,
            updated_at: now,
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
    async fn push_to_main_plans_radio_for_each_wave_in_repo() {
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

        let commands = plan_push_notifications(
            &store,
            "loopflowstudio/loopflow",
            "refs/heads/main",
            "abc",
            "def",
        )
        .await
        .expect("plan");

        let mut argvs: Vec<Vec<String>> =
            commands.into_iter().map(|command| command.args).collect();
        argvs.sort();
        assert_eq!(
            argvs,
            vec![
                vec![
                    "radio".to_string(),
                    "pub".to_string(),
                    "--channel".to_string(),
                    "ship".to_string(),
                    "--from".to_string(),
                    "github".to_string(),
                    "main moved: abc..def".to_string(),
                ],
                vec![
                    "radio".to_string(),
                    "pub".to_string(),
                    "--channel".to_string(),
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

        let commands = plan_push_notifications(
            &store,
            "loopflowstudio/loopflow",
            "refs/heads/feature",
            "abc",
            "def",
        )
        .await
        .expect("plan");
        assert!(commands.is_empty());
    }

    /// CI failure resolves the owning wave and plans the radio command carrying
    /// the wave+sha dedupe key; an unknown repo drops.
    #[tokio::test]
    async fn check_run_failure_plans_attributed_chat_with_dedupe_key() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        let wave = make_wave("ship", &repo);
        store.create_wave(&wave).await.expect("wave");
        let mut task = task_with_open_pr(&wave, 1);
        task.branch = "feature".to_string();
        store.create_task_session(&task).await.expect("task");

        let cache = Arc::new(Mutex::new(HashSet::new()));
        let event = check_run_event("loopflowstudio/loopflow", "feature", 1);

        let commands = plan_check_run_notifications(&store, &cache, &event)
            .await
            .expect("plan");
        let expected_key = format!("{}:abc123", wave.id());
        assert_eq!(
            commands,
            vec![RadioCommand::radio(
                "ship",
                "CI failed: test-check on PR #1 — https://example.test/logs".to_string(),
                "ci",
            )
            .with_dedupe_key(expected_key.clone())]
        );

        // Planning must NOT record the key — only a delivered command does.
        assert!(!cache.lock().await.contains(&expected_key));

        // Unknown repo: no wave resolves, dropped.
        let unknown = check_run_event("loopflowstudio/ghost", "feature", 1);
        let dropped = plan_check_run_notifications(&store, &cache, &unknown)
            .await
            .expect("plan unknown");
        assert!(dropped.is_empty(), "no wave resolved → drop");
    }

    /// The bounce contract: a failed command leaves the dedupe key absent so
    /// the same wave+sha replays; a delivered command records it so the replay
    /// dedupes to nothing.
    #[tokio::test]
    async fn bounced_ci_publish_leaves_key_absent_so_replay_succeeds() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        let wave = make_wave("ship", &repo);
        store.create_wave(&wave).await.expect("wave");
        let mut task = task_with_open_pr(&wave, 1);
        task.branch = "feature".to_string();
        store.create_task_session(&task).await.expect("task");

        let cache = Arc::new(Mutex::new(HashSet::new()));
        let event = check_run_event("loopflowstudio/loopflow", "feature", 1);

        // First delivery bounces (wave server down → exit ≠ 0).
        let commands = plan_check_run_notifications(&store, &cache, &event)
            .await
            .expect("plan");
        assert_eq!(commands.len(), 1);
        settle_radio_command(
            std::path::Path::new("/usr/bin/false"),
            commands[0].clone(),
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
        settle_radio_command(
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

    #[tokio::test]
    async fn merged_pr_completes_its_task_session_once() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        let wave = make_wave("ship", &repo);
        store.create_wave(&wave).await.expect("wave");
        let task = task_with_open_pr(&wave, 7);
        store.create_task_session(&task).await.expect("task");

        let processed = complete_merged_task_sessions(&store, "loopflowstudio/loopflow", 7)
            .await
            .expect("complete task");
        assert_eq!(processed, 1);
        let merged = store
            .get_task_session(&task.id)
            .await
            .expect("read task")
            .expect("task remains");
        assert_eq!(merged.status, TaskSessionStatus::Merged);
        assert!(matches!(
            merged.pm_writeback,
            PmWritebackState::Pending {
                operation: PmWritebackOperation::CompleteTask,
                ..
            }
        ));
        let (_, events) =
            crate::wave::journal::Journal::open(&crate::wave::journal::journal_path(&repo, "ship"))
                .expect("wave journal");
        assert!(events.iter().any(|event| matches!(
            &event.kind,
            crate::wave::journal::EventKind::UserMessage { text, .. }
                if text.contains("Task INF-123 → merged")
        )));

        let processed_again = complete_merged_task_sessions(&store, "loopflowstudio/loopflow", 7)
            .await
            .expect("idempotent completion");
        assert_eq!(processed_again, 0);
    }

    /// Every planned argv must resolve to a real `lf` subcommand. Bare
    /// `Cli::try_parse_from` is not enough: `lf` accepts external subcommands,
    /// so a stale verb (`op queue reconcile`) parses fine and then fails at
    /// runtime as a silent no-op. Assert we landed on a known command.
    #[test]
    fn planned_radio_commands_resolve_to_known_lf_commands() {
        use clap::Parser;

        let command = RadioCommand::radio("ship", "main moved".to_string(), "github");
        let argv = std::iter::once("lf".to_string()).chain(command.args.iter().cloned());
        let cli = crate::lf::Cli::try_parse_from(argv)
            .unwrap_or_else(|err| panic!("{:?} must parse: {err}", command.args));
        assert!(
            !matches!(cli.command, Some(crate::lf::Commands::External(_))),
            "{:?} fell through to an external subcommand — stale verb",
            command.args
        );
    }

    /// A deleted branch removes the sibling worktree that was on it, and leaves
    /// unrelated branches alone.
    #[tokio::test]
    async fn delete_branch_removes_the_matching_worktree() {
        let tmp = tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let repo = github_backed_repo(tmp.path(), "loopflowstudio/loopflow");
        let repo_str = repo.to_string_lossy().to_string();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(args)
                .status()
                .expect("git")
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["-C", &repo_str, "config", "user.email", "t@x.com"]);
        git(&["-C", &repo_str, "config", "user.name", "t"]);
        std::fs::write(repo.join("f.txt"), "x").unwrap();
        git(&["-C", &repo_str, "add", "."]);
        git(&["-C", &repo_str, "commit", "-m", "init"]);

        let wt = repo.parent().unwrap().join(format!(
            "{}.feat",
            repo.file_name().unwrap().to_string_lossy()
        ));
        git(&[
            "-C",
            &repo_str,
            "worktree",
            "add",
            "-b",
            "jack/feat",
            wt.to_str().unwrap(),
        ]);
        assert!(wt.exists());
        store
            .create_wave(&make_wave("ship", &repo))
            .await
            .expect("wave");

        let removed =
            remove_worktrees_for_deleted_branch(&store, "loopflowstudio/loopflow", "jack/feat")
                .await
                .expect("remove");
        assert_eq!(removed.len(), 1, "one worktree removed");
        assert!(!wt.exists(), "worktree gone after its branch was deleted");

        // A branch with no worktree removes nothing.
        let none =
            remove_worktrees_for_deleted_branch(&store, "loopflowstudio/loopflow", "jack/ghost")
                .await
                .expect("none");
        assert!(none.is_empty());
    }

    #[test]
    fn task_matches_ci_target_requires_matching_nonterminal_delivery() {
        let wave = make_wave("ship", std::path::Path::new("."));
        let mut task = task_with_open_pr(&wave, 1);
        task.branch = "feature".to_string();
        assert!(task_matches_ci_target(&task, Some("feature"), Some(1)));
        assert!(!task_matches_ci_target(&task, Some("other"), Some(1)));
        assert!(!task_matches_ci_target(&task, Some("feature"), Some(2)));

        task.status = TaskSessionStatus::Merged;
        assert!(!task_matches_ci_target(&task, Some("feature"), Some(1)));
    }
}
