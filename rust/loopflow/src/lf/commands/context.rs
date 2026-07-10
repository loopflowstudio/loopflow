//! `lf context` — query supplied-context evidence without opening prompt bodies.

use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde::Serialize;

use crate::journal::open_ledger;
use crate::trace::{AgentLaunchRow, AgentTurnRow, ContextAssetRow, ContextDecisionRow};

#[derive(Debug, Serialize)]
pub struct ContextDatasetDto {
    pub days: u32,
    pub turns: Vec<ContextTurnDto>,
    pub assets: Vec<ContextAssetRow>,
    pub decisions: Vec<ContextDecisionRow>,
}

#[derive(Debug, Serialize)]
pub struct ContextTurnDto {
    pub run_id: String,
    pub process_id: String,
    pub launch_id: String,
    pub turn_id: String,
    pub ordinal: i64,
    pub timestamp: i64,
    pub repo: String,
    pub wave: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub coverage: String,
    pub capture_status: String,
    pub supplied_context_tokens: i64,
    pub provider_input_tokens: Option<i64>,
    pub context_gather_ms: i64,
    pub context_render_ms: i64,
    pub context_persist_ms: i64,
    pub artifact_available: bool,
}

pub fn run(days: u32, wave: Option<&str>, repo: Option<&str>, json: bool) -> Result<()> {
    let store = open_ledger().map_err(|error| anyhow!("run ledger unavailable: {error}"))?;
    let since = time::OffsetDateTime::now_utc().unix_timestamp()
        - i64::from(days).saturating_mul(24 * 60 * 60);
    let launches = store
        .agent_launches_since(since)
        .map_err(|error| anyhow!("failed to read context launches: {error}"))?
        .into_iter()
        .filter(|launch| wave.is_none_or(|value| launch.wave.as_deref() == Some(value)))
        .filter(|launch| repo.is_none_or(|value| launch.repo == value))
        .collect::<Vec<_>>();
    let launch_ids = launches
        .iter()
        .map(|launch| launch.id.clone())
        .collect::<Vec<_>>();
    let turns = store.agent_turns_for_launches(&launch_ids)?;
    let turn_ids = turns.iter().map(|turn| turn.id.clone()).collect::<Vec<_>>();
    let assets = store.context_assets_for_turns(&turn_ids)?;
    let decisions = store.context_decisions_for_turns(&turn_ids)?;
    let rows = join_turns(&launches, &turns);
    let dataset = ContextDatasetDto {
        days,
        turns: rows,
        assets,
        decisions,
    };

    if json {
        println!("{}", serde_json::to_string(&dataset)?);
    } else {
        print_human(&dataset);
    }
    Ok(())
}

fn join_turns(launches: &[AgentLaunchRow], turns: &[AgentTurnRow]) -> Vec<ContextTurnDto> {
    let by_id: BTreeMap<&str, &AgentLaunchRow> = launches
        .iter()
        .map(|launch| (launch.id.as_str(), launch))
        .collect();
    turns
        .iter()
        .filter_map(|turn| {
            let launch = by_id.get(turn.launch_id.as_str())?;
            Some(ContextTurnDto {
                run_id: launch.run_id.clone(),
                process_id: launch.process_id.clone(),
                launch_id: launch.id.clone(),
                turn_id: turn.id.clone(),
                ordinal: turn.ordinal,
                timestamp: turn.started_at,
                repo: launch.repo.clone(),
                wave: launch.wave.clone(),
                flow: launch.flow.clone(),
                skill: launch.skill.clone(),
                provider: launch.provider.clone(),
                model: launch.model.clone(),
                coverage: turn.context_coverage.clone(),
                capture_status: launch.capture_status.clone(),
                supplied_context_tokens: turn.supplied_context_tokens,
                provider_input_tokens: turn.provider_input_tokens,
                context_gather_ms: turn.context_gather_ms,
                context_render_ms: turn.context_render_ms,
                context_persist_ms: turn.context_persist_ms,
                artifact_available: crate::trace::resolve_artifact(&turn.task_prompt_path)
                    .is_ok_and(|path| path.is_file()),
            })
        })
        .collect()
}

fn print_human(dataset: &ContextDatasetDto) {
    if dataset.turns.is_empty() {
        println!(
            "No captured agent context in the last {} days.",
            dataset.days
        );
        return;
    }
    println!("CONTEXT BY WAVE");
    let mut groups: BTreeMap<&str, Vec<&ContextTurnDto>> = BTreeMap::new();
    for turn in &dataset.turns {
        groups
            .entry(turn.wave.as_deref().unwrap_or("(no wave)"))
            .or_default()
            .push(turn);
    }
    for (wave, turns) in groups {
        let mut values = turns
            .iter()
            .filter(|turn| turn.ordinal == 1 && turn.coverage == "assembled")
            .map(|turn| turn.supplied_context_tokens)
            .collect::<Vec<_>>();
        if values.is_empty() {
            continue;
        }
        values.sort_unstable();
        let sum: i64 = values.iter().sum();
        let average = sum / values.len() as i64;
        let complete = turns
            .iter()
            .filter(|turn| turn.capture_status == "complete")
            .count();
        let partial = turns
            .iter()
            .filter(|turn| turn.capture_status == "partial")
            .count();
        let prompt_only = turns
            .iter()
            .filter(|turn| turn.capture_status == "prompt_only")
            .count();
        let provider_inputs = turns
            .iter()
            .filter_map(|turn| turn.provider_input_tokens)
            .collect::<Vec<_>>();
        let provider_average = (!provider_inputs.is_empty())
            .then(|| provider_inputs.iter().sum::<i64>() / provider_inputs.len() as i64);
        println!(
            "  {wave:<18} {:>4} launches  avg {:>8}  p50 {:>8}  p95 {:>8}  provider avg {:>8}  complete {complete} partial {partial} prompt-only {prompt_only}",
            values.len(),
            average,
            percentile(&values, 50),
            percentile(&values, 95),
            provider_average
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_string()),
        );
    }

    println!("\nASSET CONTRIBUTIONS");
    let mut kinds: BTreeMap<String, (u64, usize)> = BTreeMap::new();
    for row in &dataset.assets {
        let entry = kinds
            .entry(row.asset.kind.as_str().to_string())
            .or_default();
        entry.0 += row.asset.attributed_tokens;
        entry.1 += 1;
    }
    for (kind, (tokens, count)) in kinds {
        println!(
            "  {kind:<22} {:>10} tokens  {:>8} avg/asset  {count:>5} assets",
            tokens,
            tokens / count as u64,
        );
    }
}

fn percentile(sorted: &[i64], percent: usize) -> i64 {
    let index = (sorted.len().saturating_sub(1) * percent).div_ceil(100);
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::percentile;

    #[test]
    fn percentiles_count_each_turn_once() {
        assert_eq!(percentile(&[10, 20, 30, 40], 50), 30);
        assert_eq!(percentile(&[10, 20, 30, 40], 95), 40);
    }
}
