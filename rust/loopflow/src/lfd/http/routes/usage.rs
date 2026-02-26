use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::lfd::http::dto::{
    format_datetime, SessionUsageDto, SessionUsageSessionDto, UsageSummaryDto, WaveUsageDto,
};
use crate::lfd::http::routes::{parse_lfd_id, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::sessions::types::Session;
use crate::lfd::sessions::usage::{
    aggregate_session_events, aggregate_summary, aggregate_wave_usage, GroupBy, UsageSessionData,
};
use crate::lfd::store::SessionFilters;

#[derive(Debug, Deserialize)]
pub struct UsageSummaryQuery {
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub step: Option<String>,
    pub model: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub group_by: Option<String>,
}

pub async fn get_session_usage_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<SessionUsageDto> {
    let session_id = parse_lfd_id(&session_id, "invalid session id")?;
    let session = state
        .store
        .get_session(&session_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "session not found"))?;
    let events = state
        .store
        .list_session_events(&session_id, None)
        .await
        .map_err(map_store_error)?;
    let usage = aggregate_session_events(&events, None);

    let wave = resolve_session_wave(&state, &session).await?;

    Ok(Json(SessionUsageDto {
        object: "session_usage".to_string(),
        session_id: session_id.to_string(),
        tokens: usage.tokens,
        turns: usage.turns,
        context: usage.context,
        models: usage.models,
        session: SessionUsageSessionDto {
            step: session.config.step.clone(),
            wave,
            status: session.status.as_str().to_string(),
            created_at: format_datetime(Some(session.created_at)),
            ended_at: format_datetime(session.ended_at),
        },
    }))
}

pub async fn get_wave_usage_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<WaveUsageDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let sessions = state
        .store
        .list_sessions_for_wave(wave_id.as_str())
        .await
        .map_err(map_store_error)?;

    let mut session_events = Vec::with_capacity(sessions.len());
    for session in sessions {
        let events = state
            .store
            .list_session_events(&session.id, None)
            .await
            .map_err(map_store_error)?;
        session_events.push((session, events));
    }

    let aggregate = aggregate_wave_usage(wave_id.as_str(), &session_events);

    Ok(Json(WaveUsageDto {
        object: "wave_usage".to_string(),
        wave_id: wave_id.to_string(),
        tokens: aggregate.tokens,
        sessions: aggregate.sessions,
        turns: aggregate.turns,
        models: aggregate.models,
        by_step: aggregate.by_step,
    }))
}

pub async fn get_usage_summary_handler(
    State(state): State<HttpState>,
    Query(query): Query<UsageSummaryQuery>,
) -> ApiResult<UsageSummaryDto> {
    let group_by_raw = query
        .group_by
        .as_deref()
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "group_by is required"))?;
    let group_by = group_by_raw.parse::<GroupBy>().map_err(|_| {
        api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Safe(format!("invalid group_by: {group_by_raw}")),
        )
    })?;

    if group_by == GroupBy::Source && query.model.is_some() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "group_by=source cannot be combined with model filter",
        ));
    }

    let from = parse_datetime(query.from.as_deref(), "from")?;
    let to = parse_datetime(query.to.as_deref(), "to")?;
    if let (Some(from), Some(to)) = (from, to) {
        if from > to {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "from must be earlier than or equal to to",
            ));
        }
    }

    let wave = if let Some(wave) = query.wave.as_deref() {
        Some(resolve_wave_id(&state, wave).await?.to_string())
    } else {
        None
    };

    let filters = SessionFilters {
        wave,
        flow: query.flow.clone(),
        step: query.step.clone(),
        from: from.map(|datetime| datetime.unix_timestamp()),
        to: to.map(|datetime| datetime.unix_timestamp()),
    };
    let sessions = state
        .store
        .list_sessions_filtered(&filters)
        .await
        .map_err(map_store_error)?;

    let usage_sessions = load_usage_session_data(&state, sessions).await?;
    let groups = aggregate_summary(group_by, &usage_sessions, query.model.as_deref());

    Ok(Json(UsageSummaryDto {
        object: "usage_summary".to_string(),
        group_by: group_by.as_str().to_string(),
        from: format_datetime(from),
        to: format_datetime(to),
        groups,
    }))
}

async fn load_usage_session_data(
    state: &HttpState,
    sessions: Vec<Session>,
) -> Result<Vec<UsageSessionData>, ApiError> {
    let mut usage_sessions = Vec::with_capacity(sessions.len());

    for session in sessions {
        let events = state
            .store
            .list_session_events(&session.id, None)
            .await
            .map_err(map_store_error)?;
        let (wave_id, flow) = resolve_session_wave_run_metadata(state, &session).await?;
        usage_sessions.push(UsageSessionData {
            session,
            events,
            wave_id,
            flow,
        });
    }

    Ok(usage_sessions)
}

async fn resolve_session_wave_run_metadata(
    state: &HttpState,
    session: &Session,
) -> Result<(Option<String>, Option<String>), ApiError> {
    let Some(wave_run_id) = session.wave_run_id.as_deref() else {
        return Ok((None, None));
    };
    let Ok(wave_run_id) = wave_run_id.parse() else {
        return Ok((None, None));
    };
    let wave_run = state
        .store
        .get_wave_run(&wave_run_id)
        .await
        .map_err(map_store_error)?;
    let Some(wave_run) = wave_run else {
        return Ok((None, None));
    };

    Ok((
        Some(wave_run.wave_id.to_string()),
        Some(wave_run.snapshot.flow),
    ))
}

async fn resolve_session_wave(
    state: &HttpState,
    session: &Session,
) -> Result<Option<String>, ApiError> {
    if let Some(wave) = session.config.wave.as_ref() {
        if !wave.trim().is_empty() {
            return Ok(Some(wave.clone()));
        }
    }

    let (wave_id, _) = resolve_session_wave_run_metadata(state, session).await?;
    Ok(wave_id)
}

fn parse_datetime(value: Option<&str>, label: &str) -> Result<Option<OffsetDateTime>, ApiError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let datetime = OffsetDateTime::parse(value, &Rfc3339).map_err(|_| {
        api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Safe(format!("invalid {label} timestamp: {value}")),
        )
    })?;
    Ok(Some(datetime))
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
    use crate::lfd::sessions::types::{
        ContextSnapshot, SessionConfig, SessionEvent, SessionStatus, TurnUsage,
    };
    use crate::lfd::sessions::SessionManager;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::{
        Wave, WaveRun, WaveRunKind, WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus, WaveStatus,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tempfile::TempDir;
    use tokio::sync::Mutex;

    async fn test_http_state() -> (HttpState, TempDir) {
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
        (
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
            },
            tmp,
        )
    }

    #[tokio::test]
    async fn get_session_usage_returns_token_totals_context_and_models() {
        let (state, tmp) = test_http_state().await;
        let session = seed_session(
            &state,
            SessionConfig {
                step: "implement".to_string(),
                repo_root: tmp.path().to_string_lossy().to_string(),
                wave: Some("engbot".to_string()),
                ..Default::default()
            },
            None,
        )
        .await;

        append_context_snapshot(
            &state,
            &session.id,
            0,
            HashMap::from([("step".to_string(), 120), ("diff".to_string(), 400)]),
        )
        .await;
        append_turn_usage(
            &state,
            &session.id,
            1,
            TurnUsage {
                input_tokens: 200,
                output_tokens: 30,
                reasoning_tokens: Some(12),
                cache_read_tokens: Some(40),
                cache_write_tokens: Some(5),
                model: Some("claude-sonnet-4".to_string()),
                cost_usd: None,
            },
        )
        .await;

        let Json(payload) = get_session_usage_handler(State(state), Path(session.id.to_string()))
            .await
            .expect("session usage response");

        assert_eq!(payload.object, "session_usage");
        assert_eq!(payload.tokens.input, 200);
        assert_eq!(payload.tokens.output, 30);
        assert_eq!(payload.turns, 1);
        assert_eq!(payload.models.get("claude-sonnet-4"), Some(&1));
        assert_eq!(payload.session.wave.as_deref(), Some("engbot"));
        assert_eq!(
            payload
                .context
                .expect("context snapshot")
                .sources
                .get("diff"),
            Some(&400)
        );
    }

    #[tokio::test]
    async fn get_wave_usage_rolls_up_sessions_and_steps() {
        let (state, tmp) = test_http_state().await;
        let wave = seed_wave(&state, "engbot", &tmp).await;
        let run = seed_wave_run(&state, &wave, "build").await;
        let session = seed_session(
            &state,
            SessionConfig {
                step: "gate".to_string(),
                repo_root: tmp.path().to_string_lossy().to_string(),
                wave: Some(wave.name.clone()),
                ..Default::default()
            },
            Some(run.id.to_string()),
        )
        .await;

        append_turn_usage(
            &state,
            &session.id,
            0,
            TurnUsage {
                input_tokens: 90,
                output_tokens: 14,
                reasoning_tokens: None,
                cache_read_tokens: Some(11),
                cache_write_tokens: None,
                model: Some("claude-haiku-4-5".to_string()),
                cost_usd: None,
            },
        )
        .await;

        let Json(payload) = get_wave_usage_handler(State(state), Path(wave.name))
            .await
            .expect("wave usage response");

        assert_eq!(payload.object, "wave_usage");
        assert_eq!(payload.sessions, 1);
        assert_eq!(payload.turns, 1);
        assert_eq!(payload.tokens.input, 90);
        assert_eq!(payload.models.get("claude-haiku-4-5"), Some(&1));
        assert_eq!(
            payload.by_step.get("gate").map(|step| step.sessions),
            Some(1)
        );
    }

    #[tokio::test]
    async fn usage_summary_rejects_source_group_with_model_filter() {
        let (state, _) = test_http_state().await;

        let result = get_usage_summary_handler(
            State(state),
            Query(UsageSummaryQuery {
                wave: None,
                flow: None,
                step: None,
                model: Some("claude-sonnet-4".to_string()),
                from: None,
                to: None,
                group_by: Some("source".to_string()),
            }),
        )
        .await;

        assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
    }

    #[tokio::test]
    async fn usage_summary_applies_wave_flow_and_model_filters() {
        let (state, tmp) = test_http_state().await;
        let wave = seed_wave(&state, "engbot", &tmp).await;
        let build_run = seed_wave_run(&state, &wave, "build").await;
        let gate_run = seed_wave_run(&state, &wave, "gate").await;

        let build_session = seed_session(
            &state,
            SessionConfig {
                step: "implement".to_string(),
                repo_root: tmp.path().to_string_lossy().to_string(),
                ..Default::default()
            },
            Some(build_run.id.to_string()),
        )
        .await;
        append_turn_usage(
            &state,
            &build_session.id,
            0,
            TurnUsage {
                input_tokens: 210,
                output_tokens: 18,
                reasoning_tokens: Some(7),
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: Some("claude-sonnet-4".to_string()),
                cost_usd: None,
            },
        )
        .await;

        let gate_session = seed_session(
            &state,
            SessionConfig {
                step: "implement".to_string(),
                repo_root: tmp.path().to_string_lossy().to_string(),
                ..Default::default()
            },
            Some(gate_run.id.to_string()),
        )
        .await;
        append_turn_usage(
            &state,
            &gate_session.id,
            0,
            TurnUsage {
                input_tokens: 500,
                output_tokens: 40,
                reasoning_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: Some("claude-haiku-4-5".to_string()),
                cost_usd: None,
            },
        )
        .await;

        let from = (OffsetDateTime::now_utc() - time::Duration::days(1))
            .format(&Rfc3339)
            .expect("format from");
        let to = (OffsetDateTime::now_utc() + time::Duration::days(1))
            .format(&Rfc3339)
            .expect("format to");

        let Json(payload) = get_usage_summary_handler(
            State(state),
            Query(UsageSummaryQuery {
                wave: Some(wave.name),
                flow: Some("build".to_string()),
                step: None,
                model: Some("claude-sonnet-4".to_string()),
                from: Some(from),
                to: Some(to),
                group_by: Some("step".to_string()),
            }),
        )
        .await
        .expect("summary response");

        assert_eq!(payload.object, "usage_summary");
        assert_eq!(payload.group_by, "step");
        assert_eq!(payload.groups.len(), 1);
        assert_eq!(payload.groups[0].key, "implement");
        assert_eq!(payload.groups[0].tokens.input, 210);
        assert_eq!(payload.groups[0].turns, 1);
    }

    async fn seed_wave(state: &HttpState, name: &str, tmp: &TempDir) -> Wave {
        let wave = Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo: tmp.path().to_string_lossy().to_string(),
            flow: "build".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 1,
            created_at: Some(OffsetDateTime::now_utc()),
        };
        state.store.create_wave(&wave).await.expect("create wave");
        wave
    }

    async fn seed_wave_run(state: &HttpState, wave: &Wave, flow: &str) -> WaveRun {
        let run = WaveRun {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            snapshot: WaveRunSnapshot {
                repo: wave.repo.clone(),
                flow: flow.to_string(),
                direction: Vec::new(),
                area: Vec::new(),
                pr: None,
            },
            iteration: wave.iteration,
            step_index: 0,
            status: WaveRunStatus::Completed,
            worktree: String::new(),
            branch: String::new(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: Some(OffsetDateTime::now_utc()),
            error: None,
            flow_parents: Vec::new(),
            activation_log_id: None,
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: "stack-0".to_string(),
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
        };
        state
            .store
            .create_wave_run(&run)
            .await
            .expect("create wave run");
        run
    }

    async fn seed_session(
        state: &HttpState,
        config: SessionConfig,
        wave_run_id: Option<String>,
    ) -> Session {
        let session = Session {
            id: LfdId::new(),
            harness: "claude".to_string(),
            status: SessionStatus::Ended,
            wave_run_id,
            provider_session_id: None,
            config,
            created_at: OffsetDateTime::now_utc(),
            ended_at: Some(OffsetDateTime::now_utc()),
        };
        state
            .store
            .create_session(&session)
            .await
            .expect("create session");
        session
    }

    async fn append_turn_usage(state: &HttpState, session_id: &LfdId, seq: i64, usage: TurnUsage) {
        state
            .store
            .append_session_event(
                session_id,
                seq,
                &SessionEvent::TurnUsage {
                    turn_id: format!("turn_{seq}"),
                    usage,
                },
                OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
            .expect("append turn usage");
    }

    async fn append_context_snapshot(
        state: &HttpState,
        session_id: &LfdId,
        seq: i64,
        sources: HashMap<String, u64>,
    ) {
        state
            .store
            .append_session_event(
                session_id,
                seq,
                &SessionEvent::ContextSnapshot {
                    snapshot: ContextSnapshot {
                        sources,
                        budget: 200_000,
                        total: 520,
                        diff_tier: "UnifiedDiff".to_string(),
                    },
                },
                OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
            .expect("append context snapshot");
    }
}
