use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::lfd::http::dto::{format_datetime, ListResponse, RepoDto};
use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::types::{Repo, Wave};

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

    let repo = if let Some(existing) = state.store.get_repo(&path).await.map_err(map_store_error)? {
        existing
    } else {
        let repo = Repo {
            name: repo_name_from_path(Path::new(&path)),
            path: path.clone(),
            added_at: OffsetDateTime::now_utc(),
        };
        state
            .store
            .upsert_repo(&repo)
            .await
            .map_err(map_store_error)?;
        repo
    };

    let wave_count = state
        .store
        .list_waves(Some(&repo.path))
        .await
        .map_err(map_store_error)?
        .len() as u32;

    Ok((
        StatusCode::CREATED,
        Json(repo_to_dto(repo, wave_count)),
    ))
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
        repos.insert(
            path.clone(),
            RepoDto {
                object: "repo".to_string(),
                path,
                name,
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
        wave_count,
        registered: true,
        added_at: format_datetime(Some(repo.added_at)),
    }
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
    use crate::lfd::types::WaveStatus;
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
        }
    }

    #[test]
    fn merges_registered_and_wave_derived_repos() {
        let registered_path = "/tmp/repo-a".to_string();
        let wave_only_path = "/tmp/repo-b".to_string();
        let registered = vec![Repo {
            path: registered_path.clone(),
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
        assert_eq!(repos[0].wave_count, 2);
        assert!(repos[0].registered);
        assert!(repos[0].added_at.is_some());
        assert_eq!(repos[1].path, wave_only_path);
        assert_eq!(repos[1].wave_count, 1);
        assert!(!repos[1].registered);
        assert!(repos[1].added_at.is_none());
    }

    #[tokio::test]
    async fn add_repo_registers_git_repo() {
        let state = test_http_state().await;
        let tmp = tempdir().expect("tempdir");
        let repo_path = tmp.path().join("loopflow");
        std::fs::create_dir_all(repo_path.join(".git")).expect("create fake git repo");

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
    async fn remove_repo_unregisters_path() {
        let state = test_http_state().await;
        let tmp = tempdir().expect("tempdir");
        let repo_path = tmp.path().join("loopflow");
        std::fs::create_dir_all(repo_path.join(".git")).expect("create fake git repo");

        let canonical = std::fs::canonicalize(&repo_path)
            .expect("canonical path")
            .to_string_lossy()
            .to_string();

        state
            .store
            .upsert_repo(&Repo {
                path: canonical.clone(),
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
}
