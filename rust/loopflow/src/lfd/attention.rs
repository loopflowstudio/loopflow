use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    AttentionItem, AttentionKind, AttentionStatus, LivePrState, QueueBlock, Wave, WaveRun,
    WaveRunStatus,
};
use serde_json::json;
use time::OffsetDateTime;

/// Derive a stable attention ID from the underlying object.
///
/// Algedonic items reuse the run_id so the same block produces the same
/// attention item across reconciliation cycles.  Interactive steps generate
/// a fresh ID (the step invocation is the source of truth).
pub fn attention_id(kind: AttentionKind, _wave_id: &LfdId, run_id: Option<&LfdId>) -> LfdId {
    if matches!(kind, AttentionKind::Algedonic) {
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
    let pr = run.pr.as_ref();
    let item = AttentionItem {
        id,
        wave_id: wave.id().clone(),
        run_id: Some(run.id.clone()),
        kind: AttentionKind::InteractiveStep,
        status: AttentionStatus::Surfaced,
        title: format!("Review ready: {}", wave.name()),
        summary: pr
            .and_then(|pr| pr.title.clone())
            .unwrap_or_else(|| "Wave is ready for code review and shipping.".to_string()),
        context: json!({
            "step": "code_review",
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
        kind: AttentionKind::Algedonic,
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
            AttentionKind::Algedonic => should_resolve_algedonic(store, &item).await?,
            AttentionKind::InteractiveStep => should_resolve_interactive_step(store, &item).await?,
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

/// Algedonic items with a queue block context never auto-resolve (explicit
/// clearance required).  Step failures resolve when the run is superseded.
async fn should_resolve_algedonic(
    store: &SharedStore,
    item: &AttentionItem,
) -> Result<bool, String> {
    if item.context.get("reason").is_some() {
        return Ok(false);
    }
    should_resolve_step_failure(store, item).await
}

/// Interactive steps with PR context resolve when the PR merges/closes.
/// Others resolve when the wave restarts with a different run.
async fn should_resolve_interactive_step(
    store: &SharedStore,
    item: &AttentionItem,
) -> Result<bool, String> {
    if item.context.get("pr_url").is_some() {
        return should_resolve_code_review(store, item).await;
    }
    should_resolve_when_wave_restarted(store, item).await
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
    let Some(pr_number) = run.pr.as_ref().and_then(|pr| pr.number) else {
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
    let Some(reason_str) = item
        .context
        .get("reason")
        .and_then(serde_json::Value::as_str)
    else {
        return Ok(None);
    };
    let reason = reason_str
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
            AttentionKind::Algedonic,
            &block.wave_id,
            Some(&block.run_id),
        ),
        wave_id: block.wave_id.clone(),
        run_id: Some(block.run_id.clone()),
        kind: AttentionKind::Algedonic,
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

pub fn queue_block_attention_item_from_existing(
    block: &QueueBlock,
    existing: Option<&AttentionItem>,
) -> AttentionItem {
    let mut item = queue_block_attention_item(block);
    let Some(existing) = existing else {
        return item;
    };
    if existing.kind != AttentionKind::Algedonic || existing.status == AttentionStatus::Resolved {
        return item;
    }
    item.status = existing.status;
    item.surfaced_at = existing.surfaced_at;
    item.viewed_at = existing.viewed_at;
    item
}

#[cfg(test)]
mod tests {
    use super::{
        queue_block_attention_item, queue_block_attention_item_from_existing,
        queue_block_from_attention,
    };
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{AttentionStatus, QueueBlock, QueueBlockReason};
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

    #[test]
    fn queue_block_attention_preserves_open_lifecycle_fields() {
        let block = QueueBlock {
            wave_id: LfdId::new(),
            run_id: LfdId::new(),
            reason: QueueBlockReason::ScratchDirty,
            attempted_at: OffsetDateTime::now_utc(),
            conflict_files: Vec::new(),
            error: None,
        };
        let mut existing = queue_block_attention_item(&block);
        existing.status = AttentionStatus::Viewed;
        existing.viewed_at = Some(existing.surfaced_at + time::Duration::minutes(1));
        existing.surfaced_at -= time::Duration::hours(1);

        let refreshed = queue_block_attention_item_from_existing(&block, Some(&existing));

        assert_eq!(refreshed.status, AttentionStatus::Viewed);
        assert_eq!(refreshed.surfaced_at, existing.surfaced_at);
        assert_eq!(refreshed.viewed_at, existing.viewed_at);
    }
}
