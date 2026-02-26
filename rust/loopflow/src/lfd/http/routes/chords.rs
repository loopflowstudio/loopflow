use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use tokio_postgres::error::SqlState;

use crate::lfd::http::dto::{chord_dto, ChordDto, ListResponse};
use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::store::StoreError;

#[derive(Debug, Deserialize)]
pub struct CreateChordRequest {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AddChordMemberRequest {
    pub wave_id: String,
}

pub async fn create_chord_handler(
    State(state): State<HttpState>,
    Json(payload): Json<CreateChordRequest>,
) -> ApiResult<ChordDto> {
    let name = payload.name.trim();
    if name.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "name cannot be empty"));
    }

    match state.store.create_chord(name).await {
        Ok(chord) => Ok(Json(chord_dto(chord))),
        Err(err) if is_duplicate_chord_name_error(&err) => {
            Err(api_error(StatusCode::CONFLICT, "chord name already exists"))
        }
        Err(err) => Err(map_store_error(err)),
    }
}

pub async fn list_chords_handler(
    State(state): State<HttpState>,
) -> ApiResult<ListResponse<ChordDto>> {
    let chords = state.store.list_chords().await.map_err(map_store_error)?;
    let data = chords.into_iter().map(chord_dto).collect();
    Ok(Json(ListResponse::new(data, false)))
}

pub async fn get_chord_handler(
    State(state): State<HttpState>,
    Path(chord_id): Path<String>,
) -> ApiResult<ChordDto> {
    let chord_id = parse_lfd_id(&chord_id, "invalid chord id")?;
    let chord = state
        .store
        .get_chord(&chord_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "chord not found"))?;
    Ok(Json(chord_dto(chord)))
}

pub async fn delete_chord_handler(
    State(state): State<HttpState>,
    Path(chord_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let chord_id = parse_lfd_id(&chord_id, "invalid chord id")?;
    state
        .store
        .delete_chord(&chord_id)
        .await
        .map_err(map_store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn add_chord_member_handler(
    State(state): State<HttpState>,
    Path(chord_id): Path<String>,
    Json(payload): Json<AddChordMemberRequest>,
) -> Result<StatusCode, ApiError> {
    let chord_id = parse_lfd_id(&chord_id, "invalid chord id")?;
    let wave_id = parse_lfd_id(&payload.wave_id, "invalid wave id")?;
    state
        .store
        .add_chord_member(&chord_id, &wave_id)
        .await
        .map_err(map_store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn remove_chord_member_handler(
    State(state): State<HttpState>,
    Path((chord_id, wave_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let chord_id = parse_lfd_id(&chord_id, "invalid chord id")?;
    let wave_id = parse_lfd_id(&wave_id, "invalid wave id")?;
    state
        .store
        .remove_chord_member(&chord_id, &wave_id)
        .await
        .map_err(map_store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

fn parse_lfd_id(value: &str, error_message: &'static str) -> Result<LfdId, ApiError> {
    value
        .parse::<LfdId>()
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, error_message))
}

fn is_duplicate_chord_name_error(error: &StoreError) -> bool {
    match error {
        StoreError::Sqlite(err) => matches!(
            err,
            rusqlite::Error::SqliteFailure(_, Some(message))
                if message.contains("UNIQUE constraint failed: chords.name")
        ),
        StoreError::Postgres(err) => err.as_db_error().is_some_and(|db_error| {
            db_error.code() == &SqlState::UNIQUE_VIOLATION
                && db_error.constraint() == Some("chords_name_key")
        }),
        _ => false,
    }
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
    use crate::lfd::types::{Wave, WaveStatus};
    use std::sync::Arc;
    use tempfile::tempdir;
    use time::OffsetDateTime;
    use tokio::sync::Mutex;

    fn make_wave(repo: &str) -> Wave {
        let id = LfdId::new();
        Wave {
            id: id.clone(),
            name: format!("wave-{id}"),
            repo: repo.to_string(),
            flow: "build".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        }
    }

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
            store,
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
    async fn chord_crud_and_membership_handlers() {
        let state = test_http_state().await;
        let wave = make_wave("/repo");
        state.store.create_wave(&wave).await.expect("create wave");

        let Json(chord) = create_chord_handler(
            State(state.clone()),
            Json(CreateChordRequest {
                name: "frontend".to_string(),
            }),
        )
        .await
        .expect("create chord");
        assert_eq!(chord.name, "frontend");
        assert!(!chord.is_default);
        assert!(chord.created_at.is_some());

        let Json(chords) = list_chords_handler(State(state.clone()))
            .await
            .expect("list chords");
        assert_eq!(chords.data.len(), 1);
        assert_eq!(chords.data[0].id, chord.id);

        let Json(found) = get_chord_handler(State(state.clone()), Path(chord.id.clone()))
            .await
            .expect("get chord");
        assert_eq!(found.id, chord.id);

        let add_status = add_chord_member_handler(
            State(state.clone()),
            Path(chord.id.clone()),
            Json(AddChordMemberRequest {
                wave_id: wave.id().to_string(),
            }),
        )
        .await
        .expect("add chord member");
        assert_eq!(add_status, StatusCode::NO_CONTENT);

        let remove_status = remove_chord_member_handler(
            State(state.clone()),
            Path((chord.id.clone(), wave.id().to_string())),
        )
        .await
        .expect("remove chord member");
        assert_eq!(remove_status, StatusCode::NO_CONTENT);

        let delete_status = delete_chord_handler(State(state.clone()), Path(chord.id.clone()))
            .await
            .expect("delete chord");
        assert_eq!(delete_status, StatusCode::NO_CONTENT);

        let get_missing = get_chord_handler(State(state), Path(chord.id)).await;
        assert!(matches!(get_missing, Err((StatusCode::NOT_FOUND, _))));
    }

    #[tokio::test]
    async fn create_chord_handler_returns_conflict_for_duplicate_name() {
        let state = test_http_state().await;
        let _ = create_chord_handler(
            State(state.clone()),
            Json(CreateChordRequest {
                name: "backend".to_string(),
            }),
        )
        .await
        .expect("first create chord");

        let duplicate = create_chord_handler(
            State(state),
            Json(CreateChordRequest {
                name: "backend".to_string(),
            }),
        )
        .await;
        assert!(matches!(duplicate, Err((StatusCode::CONFLICT, _))));
    }

    #[tokio::test]
    async fn create_chord_handler_rejects_empty_name() {
        let state = test_http_state().await;

        let result = create_chord_handler(
            State(state),
            Json(CreateChordRequest {
                name: "   ".to_string(),
            }),
        )
        .await;
        assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
    }

    #[tokio::test]
    async fn chord_membership_handlers_return_not_found_for_unknown_resources() {
        let state = test_http_state().await;
        let wave_id = LfdId::new();
        let chord_id = LfdId::new();

        let add_missing_chord = add_chord_member_handler(
            State(state.clone()),
            Path(chord_id.to_string()),
            Json(AddChordMemberRequest {
                wave_id: wave_id.to_string(),
            }),
        )
        .await;
        assert!(matches!(add_missing_chord, Err((StatusCode::NOT_FOUND, _))));

        let Json(chord) = create_chord_handler(
            State(state.clone()),
            Json(CreateChordRequest {
                name: "ops".to_string(),
            }),
        )
        .await
        .expect("create chord");

        let add_missing_wave = add_chord_member_handler(
            State(state.clone()),
            Path(chord.id.clone()),
            Json(AddChordMemberRequest {
                wave_id: wave_id.to_string(),
            }),
        )
        .await;
        assert!(matches!(add_missing_wave, Err((StatusCode::NOT_FOUND, _))));

        let remove_missing_wave =
            remove_chord_member_handler(State(state), Path((chord.id, wave_id.to_string()))).await;
        assert!(matches!(
            remove_missing_wave,
            Err((StatusCode::NOT_FOUND, _))
        ));
    }

    #[tokio::test]
    async fn chord_handlers_reject_invalid_ids() {
        let state = test_http_state().await;

        let get_result =
            get_chord_handler(State(state.clone()), Path("not-an-id".to_string())).await;
        assert!(matches!(get_result, Err((StatusCode::BAD_REQUEST, _))));

        let add_result = add_chord_member_handler(
            State(state.clone()),
            Path("not-an-id".to_string()),
            Json(AddChordMemberRequest {
                wave_id: "still-not-an-id".to_string(),
            }),
        )
        .await;
        assert!(matches!(add_result, Err((StatusCode::BAD_REQUEST, _))));

        let remove_result = remove_chord_member_handler(
            State(state),
            Path(("not-an-id".to_string(), "also-not-an-id".to_string())),
        )
        .await;
        assert!(matches!(remove_result, Err((StatusCode::BAD_REQUEST, _))));
    }
}
