use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::lfd::attention::{mark_attention_viewed, resolve_attention_item};
use crate::lfd::http::dto::{attention_item_dto, AttentionItemDto, ErrorResponse, ListResponse};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::types::{AttentionItem, AttentionKind, AttentionStatus, Event};
use time::OffsetDateTime;

#[derive(Debug, Deserialize, Default)]
pub struct ListAttentionQuery {
    repo: Option<String>,
    status: Option<String>,
    kind: Option<String>,
    limit: Option<u32>,
}

pub async fn list_attention_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListAttentionQuery>,
) -> ApiResult<ListResponse<AttentionItemDto>> {
    let unresolved_only =
        query.status.as_deref().is_none() || query.status.as_deref() == Some("unresolved");
    let status = parse_status_filter(query.status.as_deref())?;
    let kind = parse_kind_filter(query.kind.as_deref())?;
    let mut items = state
        .store
        .list_attention_items(status, kind)
        .await
        .map_err(map_store_error)?;
    items = filter_items_for_repo(&state, items, query.repo.as_deref()).await?;
    if unresolved_only {
        items.retain(|item| item.status != AttentionStatus::Resolved);
    }
    sort_attention_items(&mut items);
    let limit = query.limit.unwrap_or(100) as usize;
    let has_more = items.len() > limit;
    if has_more {
        items.truncate(limit);
    }
    Ok(Json(ListResponse::new(
        items.into_iter().map(attention_item_dto).collect(),
        has_more,
    )))
}

pub async fn list_attention_history_handler(
    State(state): State<HttpState>,
    Query(mut query): Query<ListAttentionQuery>,
) -> ApiResult<ListResponse<AttentionItemDto>> {
    query.status = Some(AttentionStatus::Resolved.as_str().to_string());
    list_attention_handler(State(state), Query(query)).await
}

pub async fn get_attention_handler(
    State(state): State<HttpState>,
    Path(attention_id): Path<String>,
) -> Result<Json<AttentionItemDto>, (StatusCode, Json<ErrorResponse>)> {
    let attention_id = attention_id
        .parse::<LfdId>()
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid attention item id"))?;
    let item = state
        .store
        .get_attention_item(&attention_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "attention item not found"))?;
    Ok(Json(attention_item_dto(item)))
}

pub async fn patch_attention_handler(
    State(state): State<HttpState>,
    Path(attention_id): Path<String>,
) -> Result<Json<AttentionItemDto>, (StatusCode, Json<ErrorResponse>)> {
    let attention_id = attention_id
        .parse::<LfdId>()
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid attention item id"))?;
    let item = mark_attention_viewed(&state.store, &attention_id)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, ApiMessage::Safe(err)))?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "attention item not found"))?;
    state
        .event_hub
        .send(crate::lfd::types::Event::attention_updated(item.clone()));
    Ok(Json(attention_item_dto(item)))
}

#[derive(Debug, Deserialize)]
pub struct CreateAttentionBody {
    pub wave_id: String,
    #[serde(default)]
    pub run_id: Option<String>,
    pub kind: String,
    pub title: String,
    pub summary: String,
    #[serde(default = "default_context")]
    pub context: serde_json::Value,
}

fn default_context() -> serde_json::Value {
    serde_json::json!({})
}

/// POST /attention — create an attention item.
///
/// This is the primary API for `lf` (the CLI) to register that it needs human
/// attention. The daemon stores the item and fans it out to clients.
pub async fn create_attention_handler(
    State(state): State<HttpState>,
    Json(body): Json<CreateAttentionBody>,
) -> Result<(StatusCode, Json<AttentionItemDto>), (StatusCode, Json<ErrorResponse>)> {
    let wave_id = body
        .wave_id
        .parse::<LfdId>()
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid wave_id"))?;
    let run_id = body
        .run_id
        .map(|id| {
            id.parse::<LfdId>()
                .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid run_id"))
        })
        .transpose()?;
    let kind = body
        .kind
        .parse::<AttentionKind>()
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid attention kind"))?;

    let item = AttentionItem {
        id: LfdId::new(),
        wave_id,
        run_id,
        kind,
        status: AttentionStatus::Surfaced,
        title: body.title,
        summary: body.summary,
        context: body.context,
        surfaced_at: OffsetDateTime::now_utc(),
        viewed_at: None,
        resolved_at: None,
    };

    state
        .store
        .upsert_attention_item(&item)
        .await
        .map_err(map_store_error)?;
    state.event_hub.send(Event::attention_created(item.clone()));
    Ok((StatusCode::CREATED, Json(attention_item_dto(item))))
}

/// POST /attention/{id}/resolve — resolve an attention item.
pub async fn resolve_attention_handler(
    State(state): State<HttpState>,
    Path(attention_id): Path<String>,
) -> Result<Json<AttentionItemDto>, (StatusCode, Json<ErrorResponse>)> {
    let attention_id = attention_id
        .parse::<LfdId>()
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid attention item id"))?;
    let item = resolve_attention_item(&state.store, &attention_id)
        .await
        .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, ApiMessage::Safe(err)))?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "attention item not found"))?;
    state.event_hub.send(Event::attention_resolved(item.clone()));
    Ok(Json(attention_item_dto(item)))
}

fn parse_status_filter(
    value: Option<&str>,
) -> Result<Option<AttentionStatus>, (StatusCode, Json<ErrorResponse>)> {
    match value {
        None => Ok(None),
        Some("unresolved") => Ok(None),
        Some(raw) => raw
            .parse::<AttentionStatus>()
            .map(Some)
            .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid attention status")),
    }
}

fn parse_kind_filter(
    value: Option<&str>,
) -> Result<Option<AttentionKind>, (StatusCode, Json<ErrorResponse>)> {
    match value {
        None => Ok(None),
        Some(raw) => raw
            .parse::<AttentionKind>()
            .map(Some)
            .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid attention kind")),
    }
}

async fn filter_items_for_repo(
    state: &HttpState,
    items: Vec<AttentionItem>,
    repo: Option<&str>,
) -> Result<Vec<AttentionItem>, (StatusCode, Json<ErrorResponse>)> {
    let mut filtered = Vec::new();
    for item in items {
        let Some(repo) = repo else {
            filtered.push(item);
            continue;
        };
        let Some(wave) = state
            .store
            .get_wave(&item.wave_id)
            .await
            .map_err(map_store_error)?
        else {
            continue;
        };
        if wave.repo() == repo {
            filtered.push(item);
        }
    }
    Ok(filtered)
}

fn sort_attention_items(items: &mut [AttentionItem]) {
    items.sort_by_key(|item| {
        (
            status_weight(item.status),
            kind_weight(item.kind),
            item.surfaced_at.unix_timestamp(),
        )
    });
}

fn status_weight(status: AttentionStatus) -> u8 {
    match status {
        AttentionStatus::Surfaced => 0,
        AttentionStatus::Viewed => 1,
        AttentionStatus::Resolved => 2,
    }
}

fn kind_weight(kind: AttentionKind) -> u8 {
    match kind {
        AttentionKind::Algedonic => 0,
        AttentionKind::Interactive => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::test_http_state;
    use crate::lfd::types::{AttentionItem, AttentionKind, AttentionStatus, Wave};
    use time::OffsetDateTime;

    #[tokio::test]
    async fn list_attention_returns_sorted_unresolved_items() {
        let state = test_http_state().await;
        let wave = Wave::new(LfdId::new(), "engbot".to_string(), "/repo".to_string());
        state.store.create_wave(&wave).await.expect("create wave");

        let old = AttentionItem {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            kind: AttentionKind::Interactive,
            status: AttentionStatus::Surfaced,
            title: "code".to_string(),
            summary: "code".to_string(),
            context: serde_json::json!({}),
            surfaced_at: OffsetDateTime::now_utc() - time::Duration::hours(2),
            viewed_at: None,
            resolved_at: None,
        };
        let viewed = AttentionItem {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            kind: AttentionKind::Interactive,
            status: AttentionStatus::Viewed,
            title: "viewed".to_string(),
            summary: "viewed".to_string(),
            context: serde_json::json!({}),
            surfaced_at: OffsetDateTime::now_utc() - time::Duration::hours(3),
            viewed_at: Some(OffsetDateTime::now_utc()),
            resolved_at: None,
        };
        let resolved = AttentionItem {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            kind: AttentionKind::Algedonic,
            status: AttentionStatus::Resolved,
            title: "resolved".to_string(),
            summary: "resolved".to_string(),
            context: serde_json::json!({}),
            surfaced_at: OffsetDateTime::now_utc() - time::Duration::hours(4),
            viewed_at: None,
            resolved_at: Some(OffsetDateTime::now_utc()),
        };
        state
            .store
            .upsert_attention_item(&viewed)
            .await
            .expect("viewed");
        state.store.upsert_attention_item(&old).await.expect("old");
        state
            .store
            .upsert_attention_item(&resolved)
            .await
            .expect("resolved");

        let Json(response) = list_attention_handler(
            State(state.clone()),
            Query(ListAttentionQuery {
                repo: Some("/repo".to_string()),
                status: Some("unresolved".to_string()),
                kind: None,
                limit: None,
            }),
        )
        .await
        .expect("list attention");

        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].title, "code");
        assert_eq!(response.data[1].title, "viewed");
    }
}
