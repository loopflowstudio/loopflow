use crate::lfd::id::LfdId;
use crate::lfd::types::{AttentionItem, AttentionStatus};
use crate::lfdb::SharedStore;
use time::OffsetDateTime;

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
