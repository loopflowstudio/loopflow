use std::path::Path;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::id::WaveId;
use crate::repository::CanonicalRepo;
use crate::store::{Store, WaveLocatorUpdate};
use crate::work::wave::{Wave, WaveLocator};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaveRelocationReceipt {
    pub wave_id: String,
    pub from_repo: String,
    pub from_name: String,
    pub to_repo: String,
    pub to_name: String,
    pub waves_moved: usize,
}

#[derive(Debug, Clone)]
struct PlannedWaveMove {
    wave: Wave,
    target: WaveLocator,
    retire_collision: Option<WaveId>,
}

/// Relocation changes planning addresses. Definitions and execution retain their identities.
pub async fn relocate_wave(
    store: &Store,
    wave_id: &WaveId,
    invoking_repo: &Path,
    target_repo: Option<&Path>,
    target_name: Option<&str>,
) -> Result<WaveRelocationReceipt> {
    let wave = store
        .get_wave(wave_id)
        .await?
        .ok_or_else(|| anyhow!("Wave {wave_id} is not registered"))?;
    if wave.is_retired() {
        return Err(anyhow!("Wave {wave_id} is retired"));
    }
    let invoking = CanonicalRepo::discover(invoking_repo)?;
    let target_repo =
        CanonicalRepo::discover(target_repo.unwrap_or_else(|| Path::new(wave.repo())))?;
    let target = WaveLocator::new(target_repo, target_name.unwrap_or(wave.slug()))?;
    if let Ok(source) = CanonicalRepo::discover(Path::new(wave.repo())) {
        if source != invoking {
            return Err(anyhow!("invoke relocation from {}", wave.repo()));
        }
    } else if target.repo() != &invoking {
        return Err(anyhow!(
            "invoke relocation from its explicit target {}",
            target.repo()
        ));
    }
    ensure_repository_team_compatible(store, &wave, &target)?;
    if wave.repo() != target.repo().to_string() && wave.parent_wave_id().is_some() {
        return Err(anyhow!("repository relocation must start from a root Wave"));
    }
    let mut moves = plan_moves(store, wave, target).await?;
    for planned in &mut moves {
        if let Some(existing) = store.get_wave_at(&planned.target).await? {
            if existing.id() != planned.wave.id() {
                let blockers = store.wave_retirement_blockers(existing.id()).await?;
                if !blockers.is_empty() {
                    return Err(anyhow!(
                        "destination Wave {} retains {}",
                        existing.id(),
                        blockers.join(", ")
                    ));
                }
                planned.retire_collision = Some(existing.id().clone());
            }
        }
    }
    let root = &moves[0];
    let receipt = WaveRelocationReceipt {
        wave_id: root.wave.id().to_string(),
        from_repo: root.wave.repo().to_string(),
        from_name: root.wave.slug().to_string(),
        to_repo: root.target.repo().to_string(),
        to_name: root.target.slug().to_string(),
        waves_moved: moves.len(),
    };
    if let Some((parent, _)) = root.target.slug().rsplit_once('/') {
        store
            .sqlite
            .ensure_wave(&root.target.repo().to_string(), parent)?;
    }
    store
        .relocate_waves(
            moves
                .iter()
                .map(|planned| WaveLocatorUpdate {
                    wave_id: planned.wave.id().clone(),
                    expected_repo: planned.wave.repo().to_string(),
                    expected_slug: planned.wave.slug().to_string(),
                    target: planned.target.clone(),
                    retire_collision: planned.retire_collision.clone(),
                })
                .collect(),
        )
        .await?;
    Ok(receipt)
}

fn ensure_repository_team_compatible(
    store: &Store,
    wave: &Wave,
    target: &WaveLocator,
) -> Result<()> {
    let Ok(source) = CanonicalRepo::discover(Path::new(wave.repo())) else {
        return Ok(());
    };
    if &source == target.repo() {
        return Ok(());
    }
    let source_team =
        crate::ops::pm::repository_team_for_snapshot_validation(source.as_path(), store)
            .map_err(|error| anyhow!(error.to_string()))?;
    let target_team =
        crate::ops::pm::repository_team_for_snapshot_validation(target.repo().as_path(), store)
            .map_err(|error| anyhow!(error.to_string()))?;
    if let (Some(source_team), Some(target_team)) = (source_team, target_team) {
        if source_team != target_team {
            return Err(anyhow!(
                "cannot rehome Wave {} from repository Team {} to {}; run `lf repo reteam` explicitly before relocating",
                wave.id(),
                source_team,
                target_team
            ));
        }
    }
    Ok(())
}

async fn plan_moves(
    store: &Store,
    root: Wave,
    target: WaveLocator,
) -> Result<Vec<PlannedWaveMove>> {
    let mut moves = vec![PlannedWaveMove {
        wave: root,
        target,
        retire_collision: None,
    }];
    // Parents precede children so the store can resolve each destination parent.
    let mut next = 0;
    while next < moves.len() {
        for child in store.list_child_waves(moves[next].wave.id()).await? {
            moves.push(PlannedWaveMove {
                target: WaveLocator::new(
                    moves[next].target.repo().clone(),
                    &format!("{}/{}", moves[next].target.slug(), child.name()),
                )?,
                wave: child,
                retire_collision: None,
            });
        }
        next += 1;
    }
    Ok(moves)
}
