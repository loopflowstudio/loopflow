use axum::extract::State;
use axum::Json;

use crate::lfd::http::dto::{usage_report_dto, UsageReportDto};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{map_store_error, ApiResult};

/// Aggregated token usage across every recorded run, grouped by repo/provider,
/// wave/provider, and provider. The attribution primitive for cross-repo spend.
pub async fn usage_handler(State(state): State<HttpState>) -> ApiResult<UsageReportDto> {
    let report = state
        .store
        .aggregate_token_usage()
        .await
        .map_err(map_store_error)?;
    Ok(Json(usage_report_dto(report)))
}
