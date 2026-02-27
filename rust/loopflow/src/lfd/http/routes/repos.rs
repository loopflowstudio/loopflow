use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use time::OffsetDateTime;
use tokio_postgres::error::SqlState;

use crate::lfd::github::github_repo_from_local;
use crate::lfd::http::dto::{format_datetime, ListResponse, RepoDto};
use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::store::StoreError;
use crate::lfd::types::{Repo, RepoEdge, RepoId, Wave};

#[derive(Debug, Deserialize)]
pub struct RepoPathRequest {
    pub path: String,
}

pub async fn list_repos_handler(
    State(state): State<HttpState>,
) -> ApiResult<ListResponse<RepoDto>> {
    let registered = state.store.list_repos().await.map_err(map_store_error)?;
    let waves = state
        .store
        .list_waves(None)
        .await
        .map_err(map_store_error)?;

    let repos = build_repo_dtos(registered, waves);
    Ok(Json(ListResponse::new(repos, false)))
}

pub async fn add_repo_handler(
    State(state): State<HttpState>,
    Json(payload): Json<RepoPathRequest>,
) -> Result<(StatusCode, Json<RepoDto>), ApiError> {
    let path = validate_and_canonicalize_repo_path(&payload.path)?;
    let repo_id = github_repo_from_local(Path::new(&path)).ok_or_else(|| {
        api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "repo must have a GitHub origin remote",
        )
    })?;
    let repo_id = RepoId::parse(&repo_id)
        .map_err(|_| api_error(StatusCode::UNPROCESSABLE_ENTITY, "invalid GitHub remote"))?;

    if let Some(existing_repo) = state
        .store
        .get_repo_by_repo_id(&repo_id)
        .await
        .map_err(map_store_error)?
    {
        if existing_repo.path != path {
            return Err(api_error(
                StatusCode::CONFLICT,
                "repo already registered at a different path",
            ));
        }
    }

    let existing = state.store.get_repo(&path).await.map_err(map_store_error)?;
    let repo = Repo {
        name: repo_name_from_path(Path::new(&path)),
        path: path.clone(),
        repo_id,
        added_at: existing
            .as_ref()
            .map(|repo| repo.added_at)
            .unwrap_or_else(OffsetDateTime::now_utc),
    };

    state
        .store
        .upsert_repo(&repo)
        .await
        .map_err(|err| map_repo_upsert_error(err, repo.repo_id.as_str()))?;

    let wave_count = state
        .store
        .list_waves(Some(&repo.path))
        .await
        .map_err(map_store_error)?
        .len() as u32;

    Ok((StatusCode::CREATED, Json(repo_to_dto(repo, wave_count))))
}

pub async fn remove_repo_handler(
    State(state): State<HttpState>,
    Json(payload): Json<RepoPathRequest>,
) -> Result<StatusCode, ApiError> {
    let path = validate_absolute_repo_path(&payload.path)?;
    let normalized = if path.exists() {
        std::fs::canonicalize(&path)
            .map_err(|_| api_error(StatusCode::UNPROCESSABLE_ENTITY, "path does not exist"))?
    } else {
        path
    };
    let normalized = path_to_string(&normalized)?;

    state
        .store
        .delete_repo(&normalized)
        .await
        .map_err(map_store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_child_handler(
    State(state): State<HttpState>,
    AxumPath((owner, repo, child_owner, child_repo)): AxumPath<(String, String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let parent_repo_id = repo_id_from_segments(&owner, &repo)?;
    let child_repo_id = repo_id_from_segments(&child_owner, &child_repo)?;

    let parent = state
        .store
        .get_repo_by_repo_id(&parent_repo_id)
        .await
        .map_err(map_store_error)?;
    if parent.is_none() {
        return Err(api_error(StatusCode::NOT_FOUND, "parent repo not found"));
    }

    let child = state
        .store
        .get_repo_by_repo_id(&child_repo_id)
        .await
        .map_err(map_store_error)?;
    if child.is_none() {
        return Err(api_error(StatusCode::NOT_FOUND, "child repo not found"));
    }

    if parent_repo_id == child_repo_id {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "repo cannot be its own child",
        ));
    }

    let edges = state.store.list_edges().await.map_err(map_store_error)?;
    if would_create_cycle(&edges, &parent_repo_id, &child_repo_id) {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "edge would create a cycle",
        ));
    }

    state
        .store
        .add_edge(&RepoEdge {
            parent_repo_id,
            child_repo_id,
        })
        .await
        .map_err(map_store_error)?;

    Ok(StatusCode::OK)
}

pub async fn remove_child_handler(
    State(state): State<HttpState>,
    AxumPath((owner, repo, child_owner, child_repo)): AxumPath<(String, String, String, String)>,
) -> Result<StatusCode, ApiError> {
    let parent_repo_id = repo_id_from_segments(&owner, &repo)?;
    let child_repo_id = repo_id_from_segments(&child_owner, &child_repo)?;

    state
        .store
        .remove_edge(&parent_repo_id, &child_repo_id)
        .await
        .map_err(map_store_error)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_children_handler(
    State(state): State<HttpState>,
    AxumPath((owner, repo)): AxumPath<(String, String)>,
) -> ApiResult<ListResponse<RepoDto>> {
    let repo_id = repo_id_from_segments(&owner, &repo)?;
    let _repo = state
        .store
        .get_repo_by_repo_id(&repo_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "repo not found"))?;

    let children = state
        .store
        .children(&repo_id)
        .await
        .map_err(map_store_error)?;

    let data = build_repo_dtos_from_repos(&state, children).await?;
    Ok(Json(ListResponse::new(data, false)))
}

pub async fn list_parents_handler(
    State(state): State<HttpState>,
    AxumPath((owner, repo)): AxumPath<(String, String)>,
) -> ApiResult<ListResponse<RepoDto>> {
    let repo_id = repo_id_from_segments(&owner, &repo)?;
    let _repo = state
        .store
        .get_repo_by_repo_id(&repo_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "repo not found"))?;

    let parents = state
        .store
        .parents(&repo_id)
        .await
        .map_err(map_store_error)?;
    let data = build_repo_dtos_from_repos(&state, parents).await?;
    Ok(Json(ListResponse::new(data, false)))
}

fn build_repo_dtos(registered: Vec<Repo>, waves: Vec<Wave>) -> Vec<RepoDto> {
    let mut wave_counts: BTreeMap<String, u32> = BTreeMap::new();
    for wave in waves {
        *wave_counts.entry(wave.repo().clone()).or_insert(0) += 1;
    }

    let mut repos: BTreeMap<String, RepoDto> = BTreeMap::new();
    for repo in registered {
        let wave_count = wave_counts.remove(&repo.path).unwrap_or(0);
        repos.insert(repo.path.clone(), repo_to_dto(repo, wave_count));
    }

    for (path, wave_count) in wave_counts {
        let name = repo_name_from_path(Path::new(&path));
        let repo_id = repo_id_from_path_or_fallback(Path::new(&path));
        repos.insert(
            path.clone(),
            RepoDto {
                object: "repo".to_string(),
                path,
                name,
                repo_id,
                wave_count,
                registered: false,
                added_at: None,
            },
        );
    }

    repos.into_values().collect()
}

fn repo_to_dto(repo: Repo, wave_count: u32) -> RepoDto {
    RepoDto {
        object: "repo".to_string(),
        path: repo.path,
        name: repo.name,
        repo_id: repo.repo_id.to_string(),
        wave_count,
        registered: true,
        added_at: format_datetime(Some(repo.added_at)),
    }
}

async fn build_repo_dtos_from_repos(
    state: &HttpState,
    repos: Vec<Repo>,
) -> Result<Vec<RepoDto>, ApiError> {
    let mut result = Vec::with_capacity(repos.len());
    for repo in repos {
        let wave_count = state
            .store
            .list_waves(Some(&repo.path))
            .await
            .map_err(map_store_error)?
            .len() as u32;
        result.push(repo_to_dto(repo, wave_count));
    }
    Ok(result)
}

fn repo_id_from_segments(owner: &str, repo: &str) -> Result<RepoId, ApiError> {
    RepoId::from_owner_repo(owner, repo)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid repo identifier"))
}

fn repo_id_from_path_or_fallback(path: &Path) -> String {
    github_repo_from_local(path).unwrap_or_else(|| path.to_string_lossy().to_string())
}

fn map_repo_upsert_error(err: StoreError, repo_id: &str) -> ApiError {
    if is_repo_id_conflict(&err) {
        return api_error(
            StatusCode::CONFLICT,
            crate::lfd::http::ApiMessage::Safe(format!(
                "repo_id {repo_id} is already registered to another path"
            )),
        );
    }
    map_store_error(err)
}

fn is_repo_id_conflict(err: &StoreError) -> bool {
    match err {
        StoreError::Sqlite(sqlite_err) => matches!(
            sqlite_err,
            rusqlite::Error::SqliteFailure(_, Some(message))
                if message.contains("UNIQUE constraint failed: repos.repo_id")
        ),
        StoreError::Postgres(postgres_err) => postgres_err.as_db_error().is_some_and(|db_error| {
            db_error.code() == &SqlState::UNIQUE_VIOLATION
                && db_error.constraint() == Some("idx_repos_repo_id")
        }),
        _ => false,
    }
}

fn would_create_cycle(edges: &[RepoEdge], new_parent: &RepoId, new_child: &RepoId) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![new_child.clone()];

    while let Some(current) = stack.pop() {
        if current == *new_parent {
            return true;
        }

        if visited.insert(current.clone()) {
            for edge in edges {
                if edge.parent_repo_id == current {
                    stack.push(edge.child_repo_id.clone());
                }
            }
        }
    }

    false
}

fn validate_and_canonicalize_repo_path(raw: &str) -> Result<String, ApiError> {
    let path = validate_absolute_repo_path(raw)?;
    if !path.exists() {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "path does not exist",
        ));
    }

    let canonical = std::fs::canonicalize(&path)
        .map_err(|_| api_error(StatusCode::UNPROCESSABLE_ENTITY, "path does not exist"))?;

    if !is_git_repo(&canonical) {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "path is not a git repository",
        ));
    }

    path_to_string(&canonical)
}

fn validate_absolute_repo_path(raw: &str) -> Result<PathBuf, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "path cannot be empty",
        ));
    }

    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(api_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "path must be absolute",
        ));
    }

    Ok(path)
}

fn is_git_repo(path: &Path) -> bool {
    let git_entry = path.join(".git");
    git_entry.is_dir() || git_entry.is_file()
}

fn repo_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(std::string::ToString::to_string)
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

fn path_to_string(path: &Path) -> Result<String, ApiError> {
    path.to_str()
        .filter(|value| !value.is_empty())
        .map(std::string::ToString::to_string)
        .ok_or_else(|| api_error(StatusCode::UNPROCESSABLE_ENTITY, "path must be valid UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
    use crate::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
    use crate::lfd::events::EventHub;
    use crate::lfd::executor::WaveExecutor;
    use crate::lfd::id::LfdId;
    use crate::lfd::output::OutputHub;
    use crate::lfd::provider_auth::ProviderAuthService;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::sessions::SessionManager;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::{RepoId, WaveStatus};
    use std::process::Command;
    use std::sync::Arc;
    use tempfile::tempdir;
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

        let provider_auth = ProviderAuthService::new(store.clone());

        HttpState {
            store,
            scheduler,
            executor,
            event_hub,
            output_hub,
            provider_auth,
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

    fn make_wave(name: &str, repo: String) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo,
            flow: "build".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
        }
    }

    fn init_git_repo(path: &Path, remote: &str) {
        std::fs::create_dir_all(path).expect("create repo directory");
        let status = Command::new("git")
            .arg("init")
            .current_dir(path)
            .status()
            .expect("run git init");
        assert!(status.success());

        let status = Command::new("git")
            .args(["remote", "add", "origin", remote])
            .current_dir(path)
            .status()
            .expect("set git origin");
        assert!(status.success());
    }

    #[test]
    fn merges_registered_and_wave_derived_repos() {
        let registered_path = "/tmp/repo-a".to_string();
        let wave_only_path = "/tmp/repo-b".to_string();
        let registered = vec![Repo {
            path: registered_path.clone(),
            repo_id: RepoId::parse("loopflowstudio/repo-a").expect("repo id"),
            name: "repo-a".to_string(),
            added_at: OffsetDateTime::UNIX_EPOCH,
        }];
        let waves = vec![
            make_wave("one", registered_path.clone()),
            make_wave("two", registered_path.clone()),
            make_wave("three", wave_only_path.clone()),
        ];

        let repos = build_repo_dtos(registered, waves);

        assert_eq!(repos.len(), 2);
        assert_eq!(repos[0].path, registered_path);
        assert_eq!(repos[0].repo_id, "loopflowstudio/repo-a");
        assert_eq!(repos[0].wave_count, 2);
        assert!(repos[0].registered);
        assert!(repos[0].added_at.is_some());
        assert_eq!(repos[1].path, wave_only_path);
        assert_eq!(repos[1].repo_id, repos[1].path);
        assert_eq!(repos[1].wave_count, 1);
        assert!(!repos[1].registered);
        assert!(repos[1].added_at.is_none());
    }

    #[tokio::test]
    async fn add_repo_registers_git_repo() {
        let state = test_http_state().await;
        let tmp = tempdir().expect("tempdir");
        let repo_path = tmp.path().join("loopflow");
        init_git_repo(&repo_path, "git@github.com:loopflowstudio/loopflow.git");

        let (status, Json(dto)) = add_repo_handler(
            State(state.clone()),
            Json(RepoPathRequest {
                path: repo_path.to_string_lossy().to_string(),
            }),
        )
        .await
        .expect("add repo");

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(dto.name, "loopflow");
        assert_eq!(dto.repo_id, "loopflowstudio/loopflow");
        assert!(dto.registered);
        assert_eq!(dto.wave_count, 0);
        assert!(dto.added_at.is_some());
    }

    #[tokio::test]
    async fn add_repo_rejects_non_git_directory() {
        let state = test_http_state().await;
        let tmp = tempdir().expect("tempdir");
        let repo_path = tmp.path().join("not-a-repo");
        std::fs::create_dir_all(&repo_path).expect("create directory");

        let error = add_repo_handler(
            State(state.clone()),
            Json(RepoPathRequest {
                path: repo_path.to_string_lossy().to_string(),
            }),
        )
        .await
        .expect_err("non-git repo should fail");

        assert_eq!(error.0, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn add_repo_requires_github_remote() {
        let state = test_http_state().await;
        let tmp = tempdir().expect("tempdir");
        let repo_path = tmp.path().join("local-only");
        init_git_repo(&repo_path, "git@example.com:org/repo.git");

        let error = add_repo_handler(
            State(state.clone()),
            Json(RepoPathRequest {
                path: repo_path.to_string_lossy().to_string(),
            }),
        )
        .await
        .expect_err("non-github remote should fail");

        assert_eq!(error.0, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn remove_repo_unregisters_path() {
        let state = test_http_state().await;
        let tmp = tempdir().expect("tempdir");
        let repo_path = tmp.path().join("loopflow");
        init_git_repo(&repo_path, "git@github.com:loopflowstudio/loopflow.git");

        let canonical = std::fs::canonicalize(&repo_path)
            .expect("canonical path")
            .to_string_lossy()
            .to_string();

        state
            .store
            .upsert_repo(&Repo {
                path: canonical.clone(),
                repo_id: RepoId::parse("loopflowstudio/loopflow").expect("repo id"),
                name: "loopflow".to_string(),
                added_at: OffsetDateTime::now_utc(),
            })
            .await
            .expect("seed repo");

        let status = remove_repo_handler(
            State(state.clone()),
            Json(RepoPathRequest {
                path: canonical.clone(),
            }),
        )
        .await
        .expect("remove repo");

        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(state
            .store
            .get_repo(&canonical)
            .await
            .expect("get repo")
            .is_none());
    }

    #[tokio::test]
    async fn child_edge_crud_handlers() {
        let state = test_http_state().await;
        state
            .store
            .upsert_repo(&Repo {
                path: "/tmp/studio".to_string(),
                repo_id: RepoId::parse("loopflowstudio/studio").expect("repo id"),
                name: "studio".to_string(),
                added_at: OffsetDateTime::now_utc(),
            })
            .await
            .expect("seed parent repo");
        state
            .store
            .upsert_repo(&Repo {
                path: "/tmp/loopflow".to_string(),
                repo_id: RepoId::parse("loopflowstudio/loopflow").expect("repo id"),
                name: "loopflow".to_string(),
                added_at: OffsetDateTime::now_utc(),
            })
            .await
            .expect("seed child repo");

        let status = add_child_handler(
            State(state.clone()),
            AxumPath((
                "loopflowstudio".to_string(),
                "studio".to_string(),
                "loopflowstudio".to_string(),
                "loopflow".to_string(),
            )),
        )
        .await
        .expect("add child");
        assert_eq!(status, StatusCode::OK);

        let Json(children) = list_children_handler(
            State(state.clone()),
            AxumPath(("loopflowstudio".to_string(), "studio".to_string())),
        )
        .await
        .expect("list children");
        assert_eq!(children.data.len(), 1);
        assert_eq!(children.data[0].repo_id, "loopflowstudio/loopflow");

        let Json(parents) = list_parents_handler(
            State(state.clone()),
            AxumPath(("loopflowstudio".to_string(), "loopflow".to_string())),
        )
        .await
        .expect("list parents");
        assert_eq!(parents.data.len(), 1);
        assert_eq!(parents.data[0].repo_id, "loopflowstudio/studio");

        let status = remove_child_handler(
            State(state.clone()),
            AxumPath((
                "loopflowstudio".to_string(),
                "studio".to_string(),
                "loopflowstudio".to_string(),
                "loopflow".to_string(),
            )),
        )
        .await
        .expect("remove child");
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(state
            .store
            .list_edges()
            .await
            .expect("list edges")
            .is_empty());
    }

    #[tokio::test]
    async fn add_child_rejects_cycles() {
        let state = test_http_state().await;
        for (path, repo_id, name) in [
            ("/tmp/a", "loopflowstudio/a", "a"),
            ("/tmp/b", "loopflowstudio/b", "b"),
            ("/tmp/c", "loopflowstudio/c", "c"),
        ] {
            state
                .store
                .upsert_repo(&Repo {
                    path: path.to_string(),
                    repo_id: RepoId::parse(repo_id).expect("repo id"),
                    name: name.to_string(),
                    added_at: OffsetDateTime::now_utc(),
                })
                .await
                .expect("seed repo");
        }

        add_child_handler(
            State(state.clone()),
            AxumPath((
                "loopflowstudio".to_string(),
                "a".to_string(),
                "loopflowstudio".to_string(),
                "b".to_string(),
            )),
        )
        .await
        .expect("a->b");
        add_child_handler(
            State(state.clone()),
            AxumPath((
                "loopflowstudio".to_string(),
                "b".to_string(),
                "loopflowstudio".to_string(),
                "c".to_string(),
            )),
        )
        .await
        .expect("b->c");

        let error = add_child_handler(
            State(state.clone()),
            AxumPath((
                "loopflowstudio".to_string(),
                "c".to_string(),
                "loopflowstudio".to_string(),
                "a".to_string(),
            )),
        )
        .await
        .expect_err("c->a should be rejected");
        assert_eq!(error.0, StatusCode::UNPROCESSABLE_ENTITY);
    }
}
