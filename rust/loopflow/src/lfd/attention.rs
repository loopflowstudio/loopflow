use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    AttentionItem, AttentionKind, AttentionStatus, LivePrState, QueueBlock, Wave, WaveRun,
    WaveRunStatus,
};
use serde_json::json;
use time::OffsetDateTime;

pub fn attention_id(kind: AttentionKind, _wave_id: &LfdId, run_id: Option<&LfdId>) -> LfdId {
    if matches!(kind, AttentionKind::QueueFailure) {
        if let Some(run_id) = run_id {
            return run_id.clone();
        }
    }
    LfdId::new()
}

pub async fn create_code_review_attention(
    store: &SharedStore,
    wave: &Wave,
    run: &WaveRun,
) -> Result<AttentionItem, String> {
    let id = LfdId::new();
    let pr = run.snapshot.pr.as_ref();
    let item = AttentionItem {
        id,
        wave_id: wave.id().clone(),
        run_id: Some(run.id.clone()),
        kind: AttentionKind::CodeReview,
        status: AttentionStatus::Surfaced,
        title: format!("Review ready: {}", wave.name()),
        summary: pr
            .and_then(|pr| pr.title.clone())
            .unwrap_or_else(|| "Wave is ready for code review and shipping.".to_string()),
        context: json!({
            "pr_url": pr.map(|pr| pr.url.clone()),
            "pr_number": pr.and_then(|pr| pr.number),
            "pr_title": pr.and_then(|pr| pr.title.clone()),
            "branch": run.branch,
        }),
        surfaced_at: OffsetDateTime::now_utc(),
        viewed_at: None,
        resolved_at: None,
    };
    store
        .upsert_attention_item(&item)
        .await
        .map_err(|err| format!("upsert code review attention failed: {err}"))?;
    Ok(item)
}

pub async fn create_step_failure_attention(
    store: &SharedStore,
    wave: &Wave,
    run: &WaveRun,
    step_name: &str,
    error: &str,
) -> Result<AttentionItem, String> {
    let id = LfdId::new();
    let item = AttentionItem {
        id,
        wave_id: wave.id().clone(),
        run_id: Some(run.id.clone()),
        kind: AttentionKind::StepFailure,
        status: AttentionStatus::Surfaced,
        title: format!("Step failed: {step_name}"),
        summary: error.to_string(),
        context: json!({
            "step": step_name,
            "error": error,
        }),
        surfaced_at: OffsetDateTime::now_utc(),
        viewed_at: None,
        resolved_at: None,
    };
    store
        .upsert_attention_item(&item)
        .await
        .map_err(|err| format!("upsert step failure attention failed: {err}"))?;
    Ok(item)
}

pub async fn mark_attention_viewed(
    store: &SharedStore,
    attention_id: &LfdId,
) -> Result<Option<AttentionItem>, String> {
    let Some(mut item) = store
        .get_attention_item(attention_id)
        .await
        .map_err(|err| format!("get attention item failed: {err}"))?
    else {
        return Ok(None);
    };
    if item.status == AttentionStatus::Resolved {
        return Ok(Some(item));
    }
    if item.status == AttentionStatus::Surfaced {
        item.status = AttentionStatus::Viewed;
        item.viewed_at = Some(OffsetDateTime::now_utc());
        store
            .upsert_attention_item(&item)
            .await
            .map_err(|err| format!("update attention item failed: {err}"))?;
    }
    Ok(Some(item))
}

pub async fn reconcile_attention_items(store: &SharedStore) -> Result<Vec<AttentionItem>, String> {
    let mut resolved = Vec::new();
    let items = store
        .list_attention_items(None, None)
        .await
        .map_err(|err| format!("list attention items failed: {err}"))?;

    for mut item in items {
        if item.status == AttentionStatus::Resolved {
            continue;
        }
        let should_resolve = match item.kind {
            AttentionKind::QueueFailure => false,
            AttentionKind::CodeReview => should_resolve_code_review(store, &item).await?,
            AttentionKind::StepFailure => should_resolve_step_failure(store, &item).await?,
            AttentionKind::DesignReview => should_resolve_design_review(store, &item).await?,
            AttentionKind::Calibration => should_resolve_calibration(store, &item).await?,
        };
        if should_resolve {
            item.status = AttentionStatus::Resolved;
            item.resolved_at = Some(OffsetDateTime::now_utc());
            store
                .upsert_attention_item(&item)
                .await
                .map_err(|err| format!("resolve attention item failed: {err}"))?;
            resolved.push(item);
        }
    }

    Ok(resolved)
}

async fn should_resolve_code_review(
    store: &SharedStore,
    item: &AttentionItem,
) -> Result<bool, String> {
    let Some(run_id) = item.run_id.as_ref() else {
        return Ok(true);
    };
    let Some(run) = store
        .get_wave_run(run_id)
        .await
        .map_err(|err| format!("get wave run failed: {err}"))?
    else {
        return Ok(true);
    };
    let Some(pr_number) = run.snapshot.pr.as_ref().and_then(|pr| pr.number) else {
        return Ok(true);
    };
    let Some(state) = store
        .get_live_pr_state(&run.snapshot.repo, pr_number)
        .await
        .map_err(|err| format!("get live pr state failed: {err}"))?
    else {
        return Ok(false);
    };
    Ok(matches!(
        state.state,
        LivePrState::Closed | LivePrState::Merged
    ))
}

async fn should_resolve_step_failure(
    store: &SharedStore,
    item: &AttentionItem,
) -> Result<bool, String> {
    let Some(run_id) = item.run_id.as_ref() else {
        return Ok(true);
    };
    let Some(run) = store
        .get_wave_run(run_id)
        .await
        .map_err(|err| format!("get wave run failed: {err}"))?
    else {
        return Ok(true);
    };
    if run.status != WaveRunStatus::Failed {
        return Ok(true);
    }
    let latest = store
        .get_latest_wave_run(&item.wave_id)
        .await
        .map_err(|err| format!("get latest wave run failed: {err}"))?;
    Ok(latest.is_some_and(|latest| latest.id != run.id))
}

async fn should_resolve_design_review(
    store: &SharedStore,
    item: &AttentionItem,
) -> Result<bool, String> {
    let latest = store
        .get_latest_wave_run(&item.wave_id)
        .await
        .map_err(|err| format!("get latest wave run failed: {err}"))?;
    Ok(latest.is_some_and(|run| run.run_id_changed_from(item.run_id.as_ref())))
}

async fn should_resolve_calibration(
    store: &SharedStore,
    item: &AttentionItem,
) -> Result<bool, String> {
    let latest = store
        .get_latest_wave_run(&item.wave_id)
        .await
        .map_err(|err| format!("get latest wave run failed: {err}"))?;
    Ok(latest.is_some_and(|run| run.run_id_changed_from(item.run_id.as_ref())))
}

trait RunResolutionExt {
    fn run_id_changed_from(&self, old: Option<&LfdId>) -> bool;
}

impl RunResolutionExt for WaveRun {
    fn run_id_changed_from(&self, old: Option<&LfdId>) -> bool {
        match old {
            Some(old) => &self.id != old,
            None => true,
        }
    }
}

pub fn queue_block_from_attention(item: &AttentionItem) -> Option<QueueBlock> {
    let run_id = item.run_id.clone()?;
    let reason = item
        .context
        .get("reason")
        .and_then(serde_json::Value::as_str)?
        .parse()
        .ok()?;
    let conflict_files = item
        .context
        .get("conflict_files")
        .and_then(serde_json::Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let error = item
        .context
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    Some(QueueBlock {
        wave_id: item.wave_id.clone(),
        run_id,
        reason,
        attempted_at: item.surfaced_at,
        conflict_files,
        error,
    })
}
