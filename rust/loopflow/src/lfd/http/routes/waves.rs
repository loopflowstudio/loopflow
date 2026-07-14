use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::lfd::http::dto::{
    session_dto, ListResponse, WaveAgentTreeDto, WaveAgentTreeSessionDto, WaveDto,
};
use crate::lfd::http::routes::{build_wave_dto, resolve_wave_id};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::types::LIVE_SESSION_STATUSES;

#[derive(Debug, Deserialize)]
pub struct ListWavesQuery {
    repo: Option<String>,
    limit: Option<u32>,
    starting_after: Option<String>,
    ending_before: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetWaveAgentTreeQuery {
    active_only: Option<bool>,
}

pub async fn list_waves_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWavesQuery>,
) -> ApiResult<ListResponse<WaveDto>> {
    let waves = state
        .store
        .list_waves(query.repo.as_deref())
        .await
        .map_err(map_store_error)?;
    let (waves, has_more) = super::paginate(
        waves,
        query.limit,
        query.starting_after.as_deref(),
        query.ending_before.as_deref(),
        |w| w.id(),
    );
    let views = crate::lfd::http::routes::build_wave_dtos(waves).await;
    Ok(Json(ListResponse::new(views, has_more)))
}

pub async fn get_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<WaveDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let view = build_wave_dto(wave).await;
    Ok(Json(view))
}

pub async fn get_wave_agent_tree_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<GetWaveAgentTreeQuery>,
) -> ApiResult<WaveAgentTreeDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let root_wave = build_wave_dto(wave).await;
    let children = state
        .store
        .list_child_waves(&wave_id)
        .await
        .map_err(map_store_error)?;
    let child_waves = crate::lfd::http::routes::build_wave_dtos(children).await;
    let statuses = query
        .active_only
        .unwrap_or(true)
        .then_some(LIVE_SESSION_STATUSES);
    let sessions = state
        .store
        .list_control_sessions(Some(&wave_id), statuses)
        .await
        .map_err(map_store_error)?
        .into_iter()
        .map(|session| WaveAgentTreeSessionDto {
            session: session_dto(session),
        })
        .collect();

    Ok(Json(WaveAgentTreeDto {
        object: "wave_agent_tree".to_string(),
        id: format!("tree-{wave_id}"),
        wave: root_wave,
        child_waves,
        sessions,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::{init_git_repo, test_http_state};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Session, SessionStatus, SessionUse, Wave};
    use std::path::Path;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    fn make_wave(repo: &str, name: &str) -> Wave {
        Wave::new(LfdId::new(), name.to_string(), repo.to_string())
    }

    fn make_session(wave: &Wave, session_use: SessionUse, cwd: &Path) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use,
            skill: "implement".to_string(),
            agent: "lf".to_string(),
            cwd: cwd.to_string_lossy().to_string(),
            argv: Vec::new(),
            env: Default::default(),
            source: "wave_skill_tmux".to_string(),
            tmux_name: "lf-worker-caller".to_string(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    #[tokio::test]
    async fn get_wave_agent_tree_returns_attributed_sessions() {
        let state = test_http_state().await;
        let repo = tempdir().expect("tempdir");
        init_git_repo(repo.path());
        let wave = make_wave(&repo.path().to_string_lossy(), "tree-wave");
        state.store.create_wave(&wave).await.expect("seed wave");
        let parent = make_session(&wave, SessionUse::WaveAgent, repo.path());
        let mut child = make_session(&wave, SessionUse::Worker, repo.path());
        child.parent_session_id = Some(parent.id.clone());
        child.tmux_name = "lf-worker".to_string();
        state
            .store
            .create_control_session(&parent)
            .await
            .expect("seed parent session");
        state
            .store
            .create_control_session(&child)
            .await
            .expect("seed child session");

        let Json(response) = get_wave_agent_tree_handler(
            State(state.clone()),
            Path(wave.id().to_string()),
            Query(GetWaveAgentTreeQuery {
                active_only: Some(true),
            }),
        )
        .await
        .expect("get wave agent tree");

        assert_eq!(response.object, "wave_agent_tree");
        assert_eq!(response.wave.id, wave.id().to_string());
        // This wave is a leaf (no child waves), so the chord tree is empty here.
        assert_eq!(response.child_waves.len(), 0);
        let child_node = response
            .sessions
            .iter()
            .find(|node| node.session.id == child.id.to_string())
            .expect("child session in tree");
        assert_eq!(
            child_node.session.parent_session_id.as_deref(),
            Some(parent.id.as_str())
        );
    }

    // A chord's WaveAgentTree exposes its children: one leaf child wave per
    // repo, each with its own repo. This is the two-repo chord shape — the
    // ancestry query drives which children a chord would loop.
    #[tokio::test]
    async fn get_wave_agent_tree_returns_child_waves_for_chord() {
        let state = test_http_state().await;
        let repo_a = tempdir().expect("tempdir a");
        let repo_b = tempdir().expect("tempdir b");
        init_git_repo(repo_a.path());
        init_git_repo(repo_b.path());

        let chord = make_wave(&repo_a.path().to_string_lossy(), "chord-root");
        state.store.create_wave(&chord).await.expect("seed chord");

        let child_a = make_wave(&repo_a.path().to_string_lossy(), "chord-child-a")
            .with_parent(chord.id().clone());
        let child_b = make_wave(&repo_b.path().to_string_lossy(), "chord-child-b")
            .with_parent(chord.id().clone());
        state
            .store
            .create_wave(&child_a)
            .await
            .expect("seed child a");
        state
            .store
            .create_wave(&child_b)
            .await
            .expect("seed child b");

        let Json(response) = get_wave_agent_tree_handler(
            State(state.clone()),
            Path(chord.id().to_string()),
            Query(GetWaveAgentTreeQuery {
                active_only: Some(true),
            }),
        )
        .await
        .expect("get wave agent tree");

        assert_eq!(response.child_waves.len(), 2);
        let repos: Vec<&str> = response
            .child_waves
            .iter()
            .map(|w| w.repo.as_str())
            .collect();
        assert!(repos.contains(&repo_a.path().to_string_lossy().as_ref()));
        assert!(repos.contains(&repo_b.path().to_string_lossy().as_ref()));
    }

    #[tokio::test]
    async fn list_with_repo_filter() {
        let state = test_http_state().await;
        let repo_a_tmp = tempdir().expect("tempdir");
        let repo_b_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_a_tmp.path());
        init_git_repo(repo_b_tmp.path());
        let repo_a = repo_a_tmp.path().to_string_lossy().to_string();
        let repo_b = repo_b_tmp.path().to_string_lossy().to_string();

        state
            .store
            .create_wave(&make_wave(&repo_a, "wave-a"))
            .await
            .expect("create wave in repo a");
        state
            .store
            .create_wave(&make_wave(&repo_b, "wave-b"))
            .await
            .expect("create wave in repo b");

        let Json(listed) = list_waves_handler(
            State(state),
            Query(ListWavesQuery {
                repo: Some(repo_a.clone()),
                limit: None,
                starting_after: None,
                ending_before: None,
            }),
        )
        .await
        .expect("list waves");

        assert_eq!(listed.data.len(), 1);
        assert_eq!(listed.data[0].repo, repo_a);
        assert_eq!(listed.data[0].name, "wave-a");
    }
}
