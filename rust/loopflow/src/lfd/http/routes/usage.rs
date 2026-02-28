use std::collections::HashMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::lfd::http::dto::{
    format_datetime, SessionUsageDto, SessionUsageSessionDto, UsageSummaryDto, UsageTimeseriesDto,
    WaveUsageDto,
};
use crate::lfd::http::routes::{parse_lfd_id, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::sessions::types::Session;
use crate::lfd::sessions::usage::{
    aggregate_session_events, aggregate_summary, aggregate_timeseries, aggregate_wave_usage,
    GroupBy, TimeBucket, UsageSessionData,
};
use crate::lfd::store::SessionFilters;

#[derive(Debug, Deserialize)]
pub struct UsageQuery {
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub step: Option<String>,
    pub model: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub group_by: Option<String>,
    pub bucket: Option<String>,
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

    let session_ids: Vec<_> = sessions.iter().map(|s| s.id.clone()).collect();
    let mut events_map = state
        .store
        .list_events_for_sessions(&session_ids)
        .await
        .map_err(map_store_error)?;
    let session_events: Vec<_> = sessions
        .into_iter()
        .map(|s| {
            let events = events_map.remove(&s.id).unwrap_or_default();
            (s, events)
        })
        .collect();

    let aggregate = aggregate_wave_usage(&session_events);

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
    Query(query): Query<UsageQuery>,
) -> ApiResult<UsageSummaryDto> {
    let validated = validate_usage_query(&state, &query).await?;
    let usage_sessions = load_usage_session_data(&state, validated.sessions).await?;
    let groups = aggregate_summary(validated.group_by, &usage_sessions, query.model.as_deref());

    Ok(Json(UsageSummaryDto {
        object: "usage_summary".to_string(),
        group_by: validated.group_by.as_str().to_string(),
        from: format_datetime(validated.from),
        to: format_datetime(validated.to),
        groups,
    }))
}

pub async fn get_usage_timeseries_handler(
    State(state): State<HttpState>,
    Query(query): Query<UsageQuery>,
) -> ApiResult<UsageTimeseriesDto> {
    let bucket_raw = query
        .bucket
        .as_deref()
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "bucket is required"))?;
    let bucket = bucket_raw.parse::<TimeBucket>().map_err(|_| {
        api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Safe(format!("invalid bucket: {bucket_raw}")),
        )
    })?;

    let validated = validate_usage_query(&state, &query).await?;
    let usage_sessions = load_usage_session_data(&state, validated.sessions).await?;
    let buckets = aggregate_timeseries(
        bucket,
        validated.group_by,
        usage_sessions,
        query.model.as_deref(),
    );

    Ok(Json(UsageTimeseriesDto {
        object: "usage_timeseries".to_string(),
        bucket: bucket.as_str().to_string(),
        group_by: validated.group_by.as_str().to_string(),
        from: format_datetime(validated.from),
        to: format_datetime(validated.to),
        buckets,
    }))
}

struct ValidatedUsageQuery {
    group_by: GroupBy,
    from: Option<OffsetDateTime>,
    to: Option<OffsetDateTime>,
    sessions: Vec<Session>,
}

async fn validate_usage_query(
    state: &HttpState,
    query: &UsageQuery,
) -> Result<ValidatedUsageQuery, ApiError> {
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
        Some(resolve_wave_id(state, wave).await?.to_string())
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

    Ok(ValidatedUsageQuery {
        group_by,
        from,
        to,
        sessions,
    })
}

async fn load_usage_session_data(
    state: &HttpState,
    sessions: Vec<Session>,
) -> Result<Vec<UsageSessionData>, ApiError> {
    let session_ids: Vec<_> = sessions.iter().map(|s| s.id.clone()).collect();
    let mut events_map = state
        .store
        .list_events_for_sessions(&session_ids)
        .await
        .map_err(map_store_error)?;

    let wave_run_meta = load_wave_run_metadata(state, &sessions).await?;

    let mut usage_sessions = Vec::with_capacity(sessions.len());
    for session in sessions {
        let events = events_map.remove(&session.id).unwrap_or_default();
        let (wave_id, flow) = session
            .wave_run_id
            .as_deref()
            .and_then(|id| id.parse::<LfdId>().ok())
            .and_then(|id| wave_run_meta.get(&id))
            .cloned()
            .unwrap_or((None, None));
        usage_sessions.push(UsageSessionData {
            session,
            events,
            wave_id,
            flow,
        });
    }

    Ok(usage_sessions)
}

/// Batch-fetch wave_run metadata (wave_id, flow) for all sessions that have a wave_run_id.
/// Deduplicates IDs so each wave_run is fetched at most once.
async fn load_wave_run_metadata(
    state: &HttpState,
    sessions: &[Session],
) -> Result<HashMap<LfdId, (Option<String>, Option<String>)>, ApiError> {
    let unique_ids: std::collections::HashSet<LfdId> = sessions
        .iter()
        .filter_map(|s| s.wave_run_id.as_deref()?.parse::<LfdId>().ok())
        .collect();

    let mut meta = HashMap::with_capacity(unique_ids.len());
    for wave_run_id in unique_ids {
        let wave_run = state
            .store
            .get_wave_run(&wave_run_id)
            .await
            .map_err(map_store_error)?;
        if let Some(run) = wave_run {
            meta.insert(
                wave_run_id,
                (Some(run.wave_id.to_string()), Some(run.snapshot.flow)),
            );
        }
    }
    Ok(meta)
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

    let wave_run_id = session
        .wave_run_id
        .as_deref()
        .and_then(|id| id.parse::<LfdId>().ok());
    let Some(wave_run_id) = wave_run_id else {
        return Ok(None);
    };
    let wave_run = state
        .store
        .get_wave_run(&wave_run_id)
        .await
        .map_err(map_store_error)?;
    Ok(wave_run.map(|r| r.wave_id.to_string()))
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
        Wave, WaveMode, WaveRun, WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus, WaveStatus,
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
            Query(UsageQuery {
                wave: None,
                flow: None,
                step: None,
                model: Some("claude-sonnet-4".to_string()),
                from: None,
                to: None,
                group_by: Some("source".to_string()),
                bucket: None,
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
            Query(UsageQuery {
                wave: Some(wave.name),
                flow: Some("build".to_string()),
                step: None,
                model: Some("claude-sonnet-4".to_string()),
                from: Some(from),
                to: Some(to),
                group_by: Some("step".to_string()),
                bucket: None,
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

    #[tokio::test]
    async fn usage_timeseries_groups_by_day_and_wave() {
        let (state, tmp) = test_http_state().await;
        let wave = seed_wave(&state, "engbot", &tmp).await;
        let run = seed_wave_run(&state, &wave, "build").await;

        let first_day = parse_datetime(Some("2026-02-01T08:00:00Z"), "from")
            .expect("parse")
            .expect("date");
        let second_day = parse_datetime(Some("2026-02-02T08:00:00Z"), "from")
            .expect("parse")
            .expect("date");

        let session_a = seed_session_at(
            &state,
            SessionConfig {
                step: "implement".to_string(),
                repo_root: tmp.path().to_string_lossy().to_string(),
                ..Default::default()
            },
            Some(run.id.to_string()),
            first_day,
        )
        .await;
        append_turn_usage(
            &state,
            &session_a.id,
            0,
            TurnUsage {
                input_tokens: 140,
                output_tokens: 20,
                reasoning_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: Some("claude-sonnet-4".to_string()),
                cost_usd: None,
            },
        )
        .await;

        let session_b = seed_session_at(
            &state,
            SessionConfig {
                step: "implement".to_string(),
                repo_root: tmp.path().to_string_lossy().to_string(),
                ..Default::default()
            },
            Some(run.id.to_string()),
            second_day,
        )
        .await;
        append_turn_usage(
            &state,
            &session_b.id,
            0,
            TurnUsage {
                input_tokens: 90,
                output_tokens: 10,
                reasoning_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: Some("claude-sonnet-4".to_string()),
                cost_usd: None,
            },
        )
        .await;

        let Json(payload) = get_usage_timeseries_handler(
            State(state),
            Query(UsageQuery {
                wave: Some(wave.name),
                flow: Some("build".to_string()),
                step: None,
                model: None,
                from: Some("2026-02-01T00:00:00Z".to_string()),
                to: Some("2026-02-03T00:00:00Z".to_string()),
                group_by: Some("wave".to_string()),
                bucket: Some("day".to_string()),
            }),
        )
        .await
        .expect("timeseries response");

        assert_eq!(payload.object, "usage_timeseries");
        assert_eq!(payload.bucket, "day");
        assert_eq!(payload.group_by, "wave");
        assert_eq!(payload.buckets.len(), 2);
        assert_eq!(payload.buckets[0].period, "2026-02-01");
        assert_eq!(payload.buckets[1].period, "2026-02-02");
        assert_eq!(payload.buckets[0].groups[0].tokens.input, 140);
        assert_eq!(payload.buckets[1].groups[0].tokens.input, 90);
    }

    #[tokio::test]
    async fn usage_timeseries_rejects_invalid_bucket() {
        let (state, _) = test_http_state().await;

        let result = get_usage_timeseries_handler(
            State(state),
            Query(UsageQuery {
                wave: None,
                flow: None,
                step: None,
                model: None,
                from: None,
                to: None,
                group_by: Some("wave".to_string()),
                bucket: Some("hour".to_string()),
            }),
        )
        .await;

        assert!(matches!(result, Err((StatusCode::BAD_REQUEST, _))));
    }

    async fn seed_wave(state: &HttpState, name: &str, tmp: &TempDir) -> Wave {
        let wave = Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo: tmp.path().to_string_lossy().to_string(),
            mode: WaveMode::Loop,
            flow: "build".to_string(),
            loop_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 1,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
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
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: "stack-0".to_string(),
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
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
        seed_session_at(state, config, wave_run_id, OffsetDateTime::now_utc()).await
    }

    async fn seed_session_at(
        state: &HttpState,
        config: SessionConfig,
        wave_run_id: Option<String>,
        created_at: OffsetDateTime,
    ) -> Session {
        let session = Session {
            id: LfdId::new(),
            harness: "claude".to_string(),
            status: SessionStatus::Ended,
            wave_run_id,
            provider_session_id: None,
            config,
            created_at,
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
