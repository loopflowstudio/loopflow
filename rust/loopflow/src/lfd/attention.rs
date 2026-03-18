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
    let pr = run.pr.as_ref();
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
            "step": "code/review",
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

/// Daemon-created algedonic attention: repair chain exhausted.
pub async fn create_step_failure_attention(
    store: &SharedStore,
    wave: &Wave,
    run: &WaveRun,
    step_name: &str,
    error: &str,
) -> Result<AttentionItem, String> {
    let item = AttentionItem {
        id: LfdId::new(),
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

/// Resolve an attention item by ID.
pub async fn resolve_attention_item(
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
    item.status = AttentionStatus::Resolved;
    item.resolved_at = Some(OffsetDateTime::now_utc());
    store
        .upsert_attention_item(&item)
        .await
        .map_err(|err| format!("resolve attention item failed: {err}"))?;
    Ok(Some(item))
}

/// Map interactive step names to attention kinds for checkpoint steps.
/// Returns `None` for exploratory steps that don't gate flow progress.
fn interactive_step_kind(step_name: &str) -> Option<(AttentionKind, &'static str)> {
    match step_name {
        "review-design" | "kickoff" => Some((AttentionKind::DesignReview, "code/design")),
        "review-chord" => Some((AttentionKind::Calibration, "chord/review")),
        _ => None,
    }
}

/// Create an attention item for an interactive checkpoint step.
///
/// Returns `Ok(None)` for exploratory steps (design, explore, demo, refine)
/// that don't represent checkpoints needing queue surfacing.
pub async fn create_interactive_step_attention(
    store: &SharedStore,
    wave: &Wave,
    run: &WaveRun,
    step_name: &str,
    terminal_session_id: &LfdId,
) -> Result<Option<AttentionItem>, String> {
    let Some((kind, canonical_step)) = interactive_step_kind(step_name) else {
        return Ok(None);
    };

    let (title, summary) = match kind {
        AttentionKind::DesignReview => (
            format!("{} needs design review", wave.name()),
            format!("Interactive step '{step_name}' is waiting for design review."),
        ),
        AttentionKind::Calibration => (
            format!("{} chord review ready", wave.name()),
            format!("Interactive step '{step_name}' is waiting for chord review."),
        ),
        _ => unreachable!(),
    };

    let branch_slug = run.branch.rsplit('/').next().unwrap_or(&run.branch);

    let item = AttentionItem {
        id: LfdId::new(),
        wave_id: wave.id().clone(),
        run_id: Some(run.id.clone()),
        kind,
        status: AttentionStatus::Surfaced,
        title,
        summary,
        context: json!({
            "step": canonical_step,
            "terminal_session_id": terminal_session_id.to_string(),
            "design_path": format!("scratch/{branch_slug}.md"),
        }),
        surfaced_at: OffsetDateTime::now_utc(),
        viewed_at: None,
        resolved_at: None,
    };

    store
        .upsert_attention_item(&item)
        .await
        .map_err(|err| format!("upsert interactive step attention failed: {err}"))?;
    Ok(Some(item))
}

/// Resolve attention items for interactive steps when the terminal session completes.
pub async fn resolve_interactive_attention(
    store: &SharedStore,
    run_id: &LfdId,
) -> Result<Vec<AttentionItem>, String> {
    let items = store
        .list_attention_items(None, None)
        .await
        .map_err(|err| format!("list attention items failed: {err}"))?;

    let mut resolved = Vec::new();
    for mut item in items {
        if item.status == AttentionStatus::Resolved {
            continue;
        }
        if item.run_id.as_ref() != Some(run_id) {
            continue;
        }
        if !matches!(
            item.kind,
            AttentionKind::DesignReview | AttentionKind::Calibration
        ) {
            continue;
        }
        item.status = AttentionStatus::Resolved;
        item.resolved_at = Some(OffsetDateTime::now_utc());
        store
            .upsert_attention_item(&item)
            .await
            .map_err(|err| format!("resolve interactive attention failed: {err}"))?;
        resolved.push(item);
    }
    Ok(resolved)
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

/// Reconciliation: auto-resolve stale attention items based on context.
///
/// For algedonic items, checks whether the underlying condition has cleared:
/// - Queue blocks with `reason` in context: never auto-resolve (requires manual intervention)
/// - Items with `error` in context: resolve when the run is no longer failed or a newer run exists
/// - All others: resolve when the wave has a newer run
///
/// For interactive items: resolve when the wave has a newer run (the human moved on).
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

pub fn queue_block_attention_item_from_existing(
    block: &QueueBlock,
    existing: Option<&AttentionItem>,
) -> AttentionItem {
    let mut item = queue_block_attention_item(block);
    let Some(existing) = existing else {
        return item;
    };
    if existing.kind != AttentionKind::QueueFailure || existing.status == AttentionStatus::Resolved
    {
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
        create_interactive_step_attention, interactive_step_kind, queue_block_attention_item,
        queue_block_attention_item_from_existing, queue_block_from_attention,
        resolve_interactive_attention,
    };
    use crate::lfd::id::LfdId;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::{
        AttentionKind, AttentionStatus, QueueBlock, QueueBlockReason, Wave, WaveRun,
        WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus,
    };
    use std::sync::Arc;
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

    #[test]
    fn interactive_step_kind_maps_checkpoint_steps() {
        let (kind, step) = interactive_step_kind("review-design").unwrap();
        assert!(matches!(kind, AttentionKind::DesignReview));
        assert_eq!(step, "code/design");

        let (kind, step) = interactive_step_kind("kickoff").unwrap();
        assert!(matches!(kind, AttentionKind::DesignReview));
        assert_eq!(step, "code/design");

        let (kind, step) = interactive_step_kind("review-chord").unwrap();
        assert!(matches!(kind, AttentionKind::Calibration));
        assert_eq!(step, "chord/review");
    }

    #[test]
    fn interactive_step_kind_skips_exploratory_steps() {
        assert!(interactive_step_kind("design").is_none());
        assert!(interactive_step_kind("explore").is_none());
        assert!(interactive_step_kind("demo").is_none());
        assert!(interactive_step_kind("refine").is_none());
        assert!(interactive_step_kind("implement").is_none());
    }

    async fn test_store() -> SharedStore {
        let db_path = std::env::temp_dir().join(format!("lfd-attention-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        Arc::new(open_store(&config).await.expect("sqlite store"))
    }

    fn test_wave() -> Wave {
        Wave::new(
            LfdId::new(),
            "test-wave".to_string(),
            "/tmp/repo".to_string(),
        )
    }

    fn test_run(wave: &Wave) -> WaveRun {
        WaveRun {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            snapshot: WaveRunSnapshot {
                repo: "/tmp/repo".to_string(),
                flow: "build".to_string(),
                direction: vec![],
                area: vec![],
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Waiting,
            worktree: "/tmp/wt".to_string(),
            branch: "jack.test-wave.20260318".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: None,
            flow_parents: vec![],
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id().to_string(),
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        }
    }

    #[tokio::test]
    async fn create_interactive_step_attention_for_review_design() {
        let store = test_store().await;
        let wave = test_wave();
        store.create_wave(&wave).await.unwrap();
        let run = test_run(&wave);
        store.create_wave_run(&run).await.unwrap();
        let session_id = LfdId::new();

        let item =
            create_interactive_step_attention(&store, &wave, &run, "review-design", &session_id)
                .await
                .unwrap()
                .expect("review-design should create an attention item");

        assert_eq!(item.kind, AttentionKind::DesignReview);
        assert_eq!(item.status, AttentionStatus::Surfaced);
        assert_eq!(item.context["step"], "code/design");
        assert_eq!(item.context["terminal_session_id"], session_id.to_string());
        assert!(item.title.contains("design review"));
    }

    #[tokio::test]
    async fn create_interactive_step_attention_for_review_chord() {
        let store = test_store().await;
        let wave = test_wave();
        store.create_wave(&wave).await.unwrap();
        let run = test_run(&wave);
        store.create_wave_run(&run).await.unwrap();
        let session_id = LfdId::new();

        let item =
            create_interactive_step_attention(&store, &wave, &run, "review-chord", &session_id)
                .await
                .unwrap()
                .expect("review-chord should create an attention item");

        assert_eq!(item.kind, AttentionKind::Calibration);
        assert_eq!(item.context["step"], "chord/review");
        assert_eq!(item.context["terminal_session_id"], session_id.to_string());
        assert!(item.title.contains("chord review"));
    }

    #[tokio::test]
    async fn create_interactive_step_attention_skips_exploratory() {
        let store = test_store().await;
        let wave = test_wave();
        let run = test_run(&wave);
        let session_id = LfdId::new();

        let result = create_interactive_step_attention(&store, &wave, &run, "design", &session_id)
            .await
            .unwrap();
        assert!(result.is_none());

        let result = create_interactive_step_attention(&store, &wave, &run, "explore", &session_id)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn resolve_interactive_attention_resolves_matching_items() {
        let store = test_store().await;
        let wave = test_wave();
        store.create_wave(&wave).await.unwrap();
        let run = test_run(&wave);
        store.create_wave_run(&run).await.unwrap();
        let session_id = LfdId::new();

        // Create a design review attention item
        let item =
            create_interactive_step_attention(&store, &wave, &run, "review-design", &session_id)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(item.status, AttentionStatus::Surfaced);

        // Resolve
        let resolved = resolve_interactive_attention(&store, &run.id)
            .await
            .unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, item.id);
        assert_eq!(resolved[0].status, AttentionStatus::Resolved);
        assert!(resolved[0].resolved_at.is_some());

        // Resolve again should return empty (already resolved)
        let resolved_again = resolve_interactive_attention(&store, &run.id)
            .await
            .unwrap();
        assert!(resolved_again.is_empty());
    }

    #[test]
    fn code_review_attention_includes_step_field() {
        // Verify the context JSON shape for code review includes "step"
        let context = serde_json::json!({
            "step": "code/review",
            "pr_url": "https://github.com/org/repo/pull/1",
            "pr_number": 1,
            "pr_title": "Test PR",
            "branch": "main",
        });
        assert_eq!(context["step"], "code/review");
    }
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

pub fn queue_block_attention_item_from_existing(
    block: &QueueBlock,
    existing: Option<&AttentionItem>,
) -> AttentionItem {
    let mut item = queue_block_attention_item(block);
    let Some(existing) = existing else {
        return item;
    };
    if existing.kind != AttentionKind::QueueFailure || existing.status == AttentionStatus::Resolved {
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
        queue_block_from_attention, resolve_attention_item,
    };
    use crate::lfd::id::LfdId;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::{
        AttentionItem, AttentionKind, AttentionStatus, QueueBlock, QueueBlockReason,
    };
    use std::sync::Arc;
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
        let restored = queue_block_from_attention(&item).expect("queue block exists");

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

    async fn test_store() -> SharedStore {
        let db_path = std::env::temp_dir().join(format!("lfd-attention-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        Arc::new(open_store(&config).await.expect("sqlite store"))
    }

    #[tokio::test]
    async fn resolve_attention_item_by_id() {
        use crate::lfd::types::Wave;
        let store = test_store().await;
        let wave_id = LfdId::new();
        let wave = Wave::new(wave_id.clone(), "test".to_string(), "/tmp/repo".to_string());
        store.create_wave(&wave).await.unwrap();
        let item = AttentionItem {
            id: LfdId::new(),
            wave_id,
            run_id: None,
            kind: AttentionKind::Interactive,
            status: AttentionStatus::Surfaced,
            title: "test".to_string(),
            summary: "test".to_string(),
            context: serde_json::json!({}),
            surfaced_at: OffsetDateTime::now_utc(),
            viewed_at: None,
            resolved_at: None,
        };
        store.upsert_attention_item(&item).await.unwrap();

        let resolved = resolve_attention_item(&store, &item.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resolved.status, AttentionStatus::Resolved);
        assert!(resolved.resolved_at.is_some());

        // Resolving again returns the already-resolved item
        let again = resolve_attention_item(&store, &item.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(again.status, AttentionStatus::Resolved);
    }
}
