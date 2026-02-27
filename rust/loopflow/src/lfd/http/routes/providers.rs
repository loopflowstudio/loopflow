use axum::extract::State;
use axum::Json;

use crate::lfd::http::dto::ListResponse;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::ApiResult;
use crate::lfd::providers::{merge_auth, PROVIDER_CATALOG};

pub async fn list_providers_handler(
    State(state): State<HttpState>,
) -> ApiResult<ListResponse<crate::lfd::providers::ProviderInfoDto>> {
    let snapshots = state
        .provider_auth
        .list_statuses()
        .await
        .unwrap_or_default();

    Ok(Json(ListResponse::new(
        merge_auth(PROVIDER_CATALOG, &snapshots),
        false,
    )))
}
