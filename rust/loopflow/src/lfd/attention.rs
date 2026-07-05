use crate::lfd::id::LfdId;
use crate::lfd::types::{
    AttentionItem, AttentionKind, AttentionStatus, QueueBlock, QueueBlockReason,
};
use crate::lfdb::SharedStore;
use serde_json::json;
use time::OffsetDateTime;

/// Stable ID for queue-block attention items: reuse the run_id so upserts converge.
pub fn attention_id_for_queue_block(run_id: &LfdId) -> LfdId {
    run_id.clone()
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

// Queue block <-> attention item conversion helpers.

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
        id: attention_id_for_queue_block(&block.run_id),
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
    use crate::lfd::types::{AttentionKind, AttentionStatus, QueueBlock, QueueBlockReason};
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
        assert_eq!(item.kind, AttentionKind::Algedonic);
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
