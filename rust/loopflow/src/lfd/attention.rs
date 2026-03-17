use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    AttentionItem, AttentionKind, AttentionStatus, LivePrState, QueueBlock, QueueBlockReason, Wave,
    WaveRun, WaveRunStatus,
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
            AttentionKind::DesignReview | AttentionKind::Calibration => {
                should_resolve_when_wave_restarted(store, &item).await?
            }
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

async fn should_resolve_when_wave_restarted(
    store: &SharedStore,
    item: &AttentionItem,
) -> Result<bool, String> {
    let latest = store
        .get_latest_wave_run(&item.wave_id)
        .await
        .map_err(|err| format!("get latest wave run failed: {err}"))?;
    Ok(latest.is_some_and(|run| run_id_changed_from(&run, item.run_id.as_ref())))
}

fn run_id_changed_from(run: &WaveRun, previous_run_id: Option<&LfdId>) -> bool {
    match previous_run_id {
        Some(previous_run_id) => &run.id != previous_run_id,
        None => true,
    }
}

pub fn queue_block_from_attention(item: &AttentionItem) -> Result<Option<QueueBlock>, String> {
    let Some(run_id) = item.run_id.clone() else {
        return Ok(None);
    };
    let reason = item
        .context
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or(QueueBlockReason::PromotionFailed.as_str())
        .parse()
        .map_err(|err| format!("invalid queue block reason: {err}"))?;
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
    Ok(Some(QueueBlock {
        wave_id: item.wave_id.clone(),
        run_id,
        reason,
        attempted_at: item.surfaced_at,
        conflict_files,
        error,
    }))
}

pub fn queue_block_attention_item(block: &QueueBlock) -> AttentionItem {
    AttentionItem {
        id: attention_id(
            AttentionKind::QueueFailure,
            &block.wave_id,
            Some(&block.run_id),
        ),
        wave_id: block.wave_id.clone(),
        run_id: Some(block.run_id.clone()),
        kind: AttentionKind::QueueFailure,
        status: AttentionStatus::Surfaced,
        title: format!("Queue blocked: {}", block.reason.as_str().replace('_', " ")),
        summary: block
            .error
            .clone()
            .unwrap_or_else(|| "Queue requires attention before it can advance.".to_string()),
        context: json!({
            "reason": block.reason.as_str(),
            "conflict_files": block.conflict_files,
            "error": block.error,
        }),
        surfaced_at: block.attempted_at,
        viewed_at: None,
        resolved_at: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{queue_block_attention_item, queue_block_from_attention};
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{QueueBlock, QueueBlockReason};
    use time::OffsetDateTime;

    #[test]
    fn queue_block_helpers_round_trip() {
        let block = QueueBlock {
            wave_id: LfdId::new(),
            run_id: LfdId::new(),
            reason: QueueBlockReason::RebaseConflict,
            attempted_at: OffsetDateTime::now_utc(),
            conflict_files: vec!["src/lib.rs".to_string()],
            error: Some("merge failed".to_string()),
        };

        let item = queue_block_attention_item(&block);
        let restored = queue_block_from_attention(&item)
            .expect("queue block context parses")
            .expect("queue block exists");

        assert_eq!(restored.wave_id, block.wave_id);
        assert_eq!(restored.run_id, block.run_id);
        assert_eq!(restored.reason, block.reason);
        assert_eq!(restored.attempted_at, block.attempted_at);
        assert_eq!(restored.conflict_files, block.conflict_files);
        assert_eq!(restored.error, block.error);
    }
}
