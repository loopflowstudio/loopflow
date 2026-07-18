//! `lf context` — aggregate supplied-context evidence without opening prompt bodies.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::journal::open_ledger;
use crate::trace::{AgentLaunchRow, AgentTurnRow, ContextAssetKind, ContextAssetRow};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSetQuery {
    pub repo_paths: Vec<String>,
    pub started_after: i64,
    pub started_before: i64,
    pub waves: Vec<String>,
    pub projects: Vec<String>,
    pub tasks: Vec<String>,
    pub flows: Vec<String>,
    pub skills: Vec<String>,
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub surfaces: Vec<String>,
    pub outcomes: Vec<String>,
    pub capture_states: Vec<String>,
    pub steered_only: bool,
    pub current_revision_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextLabSnapshot {
    pub query: SessionSetQuery,
    pub coverage: ContextCoverageDto,
    pub totals: SessionSetTotals,
    pub aggregate_root: ContextFlameNode,
    pub sessions: Vec<SessionLane>,
    pub sources: Vec<InstructionSourceSummary>,
    pub evidence: Vec<SourceEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextCoverageDto {
    pub complete_launches: u64,
    pub partial_launches: u64,
    pub prompt_only_launches: u64,
    pub capturing_launches: u64,
    pub attributed_turns: u64,
    pub provider_total_only_turns: u64,
    pub unknown_turns: u64,
    pub prompt_artifacts_available: u64,
    pub conversations_available: u64,
    pub source_observable_agent_sessions: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSetTotals {
    pub runs: u64,
    pub agent_sessions: u64,
    pub turns: u64,
    pub initial_prompt_tokens: Option<u64>,
    pub initial_prompt_agent_sessions: u64,
    pub median_initial_prompt_tokens: Option<u64>,
    pub p95_initial_prompt_tokens: Option<u64>,
    pub instruction_tokens: Option<u64>,
    pub lifetime_input_tokens: Option<u64>,
    pub lifetime_input_agent_sessions: u64,
    pub median_lifetime_input_tokens: Option<u64>,
    pub p95_lifetime_input_tokens: Option<u64>,
    pub median_peak_context_percent: Option<f64>,
    pub p95_peak_context_percent: Option<f64>,
    pub peak_context_agent_sessions: u64,
    pub completed_launches: u64,
    pub failed_launches: u64,
    pub interrupted_launches: u64,
    pub running_launches: u64,
    pub steering_turns: u64,
    pub steered_launches: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContextFlameLevel {
    SessionSet,
    Kind,
    Source,
    Revision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextFlameNode {
    pub id: String,
    pub level: ContextFlameLevel,
    pub kind: Option<ContextAssetKind>,
    pub label: String,
    pub source_path: Option<String>,
    pub content_sha256: Option<String>,
    pub attributed_tokens: u64,
    pub run_count: u64,
    pub agent_session_count: u64,
    pub turn_count: u64,
    pub children: Vec<ContextFlameNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionLane {
    pub id: String,
    pub run_id: String,
    pub started_at: i64,
    pub outcome: String,
    pub steering_turns: Option<u64>,
    pub lifetime_input_tokens: Option<u64>,
    pub peak_context_percent: Option<f64>,
    pub provider: String,
    pub model: Option<String>,
    pub surface: String,
    pub wave: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub turns: Vec<TurnLane>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TurnLane {
    pub id: String,
    pub ordinal: u64,
    pub supplied_context_tokens: Option<u64>,
    pub assets: Vec<ContextLaneAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextLaneAsset {
    pub node_id: String,
    pub kind: ContextAssetKind,
    pub label: String,
    pub attributed_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstructionSourceSummary {
    pub id: String,
    pub label: String,
    pub kind: ContextAssetKind,
    pub source_path: String,
    pub impressions: Option<u64>,
    pub observed_revisions: u64,
    pub last_seen: Option<i64>,
    pub current_revision_node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TraceAddress {
    pub run_id: String,
    pub launch_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRole {
    SmoothComplete,
    HighContextComplete,
    FailedOrSteered,
    Recent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepresentativeTrace {
    pub role: EvidenceRole,
    pub address: TraceAddress,
    pub outcome: String,
    pub supplied_context_tokens: Option<u64>,
    pub selected_source_tokens: u64,
    pub prompt_artifact_available: bool,
    pub conversation_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceMeasurements {
    pub exposed_sessions: u64,
    pub exposed_launches: u64,
    pub exposed_turns: u64,
    pub attributed_tokens: u64,
    pub median_tokens_per_exposed_turn: Option<u64>,
    pub p95_tokens_per_exposed_turn: Option<u64>,
    pub first_seen: Option<i64>,
    pub last_seen: Option<i64>,
    pub completed_launches: u64,
    pub failed_launches: u64,
    pub steering_turns: Option<u64>,
    pub complete_capture_launches: u64,
    pub provider_models: Vec<ProviderModelExposure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderModelExposure {
    pub provider: String,
    pub model: Option<String>,
    pub exposed_launches: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceEvidence {
    pub node_id: String,
    pub label: String,
    pub kind: ContextAssetKind,
    pub source_path: Option<String>,
    pub content_sha256: String,
    pub current_content_sha256: Option<String>,
    pub current_source_sha256: Option<String>,
    pub precedence_layers: Vec<String>,
    pub measurements: SourceMeasurements,
    pub representatives: Vec<RepresentativeTrace>,
}

#[derive(Debug, Clone)]
pub struct ContextQueryOptions {
    pub days: u32,
    pub started_after: Option<i64>,
    pub started_before: Option<i64>,
    pub repo_paths: Vec<String>,
    pub waves: Vec<String>,
    pub projects: Vec<String>,
    pub tasks: Vec<String>,
    pub flows: Vec<String>,
    pub skills: Vec<String>,
    pub providers: Vec<String>,
    pub models: Vec<String>,
    pub surfaces: Vec<String>,
    pub outcomes: Vec<String>,
    pub capture_states: Vec<String>,
    pub steered_only: bool,
    pub current_revision_only: bool,
    pub json: bool,
}

pub fn run(options: ContextQueryOptions) -> Result<()> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let started_before = options.started_before.unwrap_or(now);
    let started_after = options
        .started_after
        .unwrap_or_else(|| started_before - i64::from(options.days).saturating_mul(24 * 60 * 60));
    if started_after >= started_before {
        return Err(anyhow!("context window must start before it ends"));
    }
    let query = SessionSetQuery {
        repo_paths: options.repo_paths,
        started_after,
        started_before,
        waves: options.waves,
        projects: options.projects,
        tasks: options.tasks,
        flows: options.flows,
        skills: options.skills,
        providers: options.providers,
        models: options.models,
        surfaces: options.surfaces,
        outcomes: options.outcomes,
        capture_states: options.capture_states,
        steered_only: options.steered_only,
        current_revision_only: options.current_revision_only,
    };
    validate_query(&query)?;

    let store = open_ledger().map_err(|error| anyhow!("run ledger unavailable: {error}"))?;
    let launches = store
        .agent_launches_since(query.started_after)
        .map_err(|error| anyhow!("failed to read context launches: {error}"))?
        .into_iter()
        .filter(|launch| launch.started_at < query.started_before)
        .filter(|launch| launch_matches(launch, &query))
        .collect::<Vec<_>>();
    let launch_ids = launches
        .iter()
        .map(|launch| launch.id.clone())
        .collect::<Vec<_>>();
    let turns = store.agent_turns_for_launches(&launch_ids)?;
    let turn_ids = turns.iter().map(|turn| turn.id.clone()).collect::<Vec<_>>();
    let assets = store.context_assets_for_turns(&turn_ids)?;
    let snapshot = aggregate(query, launches, turns, assets);

    if options.json {
        println!("{}", serde_json::to_string(&snapshot)?);
    } else {
        print_human(&snapshot);
    }
    Ok(())
}

fn validate_query(query: &SessionSetQuery) -> Result<()> {
    validate_values(
        "outcome",
        &query.outcomes,
        &["running", "completed", "failed", "interrupted"],
    )?;
    validate_values(
        "capture state",
        &query.capture_states,
        &["capturing", "complete", "partial", "prompt_only"],
    )
}

fn validate_values(label: &str, values: &[String], allowed: &[&str]) -> Result<()> {
    if let Some(value) = values
        .iter()
        .find(|value| !allowed.contains(&value.as_str()))
    {
        return Err(anyhow!("unknown {label} '{value}'"));
    }
    Ok(())
}

fn launch_matches(launch: &AgentLaunchRow, query: &SessionSetQuery) -> bool {
    matches_filter(&query.repo_paths, Some(&launch.repo))
        && matches_filter(&query.waves, launch.wave.as_ref())
        && matches_filter(&query.projects, launch.project.as_ref())
        && matches_filter(&query.tasks, launch.task.as_ref())
        && matches_filter(&query.flows, launch.flow.as_ref())
        && matches_filter(&query.skills, launch.skill.as_ref())
        && matches_filter(&query.providers, Some(&launch.provider))
        && matches_filter(&query.models, launch.model.as_ref())
        && matches_filter(&query.surfaces, Some(&launch.surface))
        && matches_filter(&query.outcomes, Some(&launch.outcome))
        && matches_filter(&query.capture_states, Some(&launch.capture_status))
}

fn matches_filter(values: &[String], candidate: Option<&String>) -> bool {
    values.is_empty() || candidate.is_some_and(|candidate| values.contains(candidate))
}

pub fn aggregate(
    query: SessionSetQuery,
    launches: Vec<AgentLaunchRow>,
    turns: Vec<AgentTurnRow>,
    assets: Vec<ContextAssetRow>,
) -> ContextLabSnapshot {
    let (mut launches, turns, assets) = filter_research_state(&query, launches, turns, assets);
    launches.sort_by_key(|launch| (launch.started_at, launch.id.clone()));
    let initial_prompt_turns = turns
        .iter()
        .filter(|turn| turn.input_op == "initial" && turn.context_coverage == "assembled")
        .map(|turn| turn.id.as_str())
        .collect::<HashSet<_>>();
    let mut assets = assets
        .into_iter()
        .filter(|row| initial_prompt_turns.contains(row.turn_id.as_str()))
        .collect::<Vec<_>>();
    let catalog_sources = discover_instruction_sources(&query);
    attach_catalog_source_paths(&mut assets, &catalog_sources);
    let launch_by_id = launches
        .iter()
        .map(|launch| (launch.id.as_str(), launch))
        .collect::<HashMap<_, _>>();
    let turn_by_id = turns
        .iter()
        .map(|turn| (turn.id.as_str(), turn))
        .collect::<HashMap<_, _>>();
    let assets_by_turn = group_assets(&assets);
    let revisions = build_revisions(&assets, &launch_by_id, &turn_by_id, &turns);
    let usage_by_launch = build_agent_session_usage(&launches, &turns);
    let initial_turns = turns
        .iter()
        .filter(|turn| initial_prompt_turns.contains(turn.id.as_str()))
        .cloned()
        .collect::<Vec<_>>();

    let sessions = build_session_lanes(&launches, &turns, &assets_by_turn, &usage_by_launch);
    let coverage = build_coverage(&launches, &turns, &assets);
    let totals = build_totals(&launches, &turns, &assets, &usage_by_launch);
    let aggregate_root = build_flame(&launches, &initial_turns, &revisions);
    let mut evidence = build_evidence(revisions);
    let sources = build_instruction_sources(&catalog_sources, &aggregate_root, &mut evidence);
    ContextLabSnapshot {
        query,
        coverage,
        totals,
        aggregate_root,
        sessions,
        sources,
        evidence,
    }
}

fn filter_research_state(
    query: &SessionSetQuery,
    mut launches: Vec<AgentLaunchRow>,
    mut turns: Vec<AgentTurnRow>,
    mut assets: Vec<ContextAssetRow>,
) -> (Vec<AgentLaunchRow>, Vec<AgentTurnRow>, Vec<ContextAssetRow>) {
    if !query.steered_only && !query.current_revision_only {
        return (launches, turns, assets);
    }

    let mut included_launches = launches
        .iter()
        .map(|launch| launch.id.clone())
        .collect::<HashSet<_>>();
    if query.steered_only {
        let steered_launches = turns
            .iter()
            .filter(|turn| turn.input_op == "steer")
            .map(|turn| turn.launch_id.clone())
            .collect::<HashSet<_>>();
        included_launches.retain(|launch_id| steered_launches.contains(launch_id));
    }
    if query.current_revision_only {
        let current_launches = launches_with_current_file_revision(&launches, &turns, &assets);
        included_launches.retain(|launch_id| current_launches.contains(launch_id));
    }

    launches.retain(|launch| included_launches.contains(&launch.id));
    turns.retain(|turn| included_launches.contains(&turn.launch_id));
    let included_turns = turns
        .iter()
        .map(|turn| turn.id.as_str())
        .collect::<HashSet<_>>();
    assets.retain(|asset| included_turns.contains(asset.turn_id.as_str()));
    (launches, turns, assets)
}

fn launches_with_current_file_revision(
    launches: &[AgentLaunchRow],
    turns: &[AgentTurnRow],
    assets: &[ContextAssetRow],
) -> HashSet<String> {
    let launch_by_id = launches
        .iter()
        .map(|launch| (launch.id.as_str(), launch))
        .collect::<HashMap<_, _>>();
    let launch_id_by_turn = turns
        .iter()
        .map(|turn| (turn.id.as_str(), turn.launch_id.as_str()))
        .collect::<HashMap<_, _>>();
    let mut current_hashes: BTreeMap<(ContextAssetKind, String), Option<String>> = BTreeMap::new();
    let mut current_launches = HashSet::new();

    for row in assets.iter().filter(|row| {
        is_current_file_revision_kind(row.asset.kind) && row.asset.source_path.is_some()
    }) {
        let Some(launch_id) = launch_id_by_turn.get(row.turn_id.as_str()) else {
            continue;
        };
        let Some(launch) = launch_by_id.get(launch_id) else {
            continue;
        };
        let canonical = canonical_identity(launch, row);
        let Some(path) = canonical.path else {
            continue;
        };
        let current_hash = current_hashes
            .entry((row.asset.kind, path.clone()))
            .or_insert_with(|| {
                hash_current_source(row.asset.kind, &path).map(|hashes| hashes.effective)
            });
        if current_hash.as_deref() == Some(row.asset.content_sha256.as_str()) {
            current_launches.insert((*launch_id).to_string());
        }
    }
    current_launches
}

fn is_current_file_revision_kind(kind: ContextAssetKind) -> bool {
    is_instruction_kind(kind) && !matches!(kind, ContextAssetKind::Goal | ContextAssetKind::Memory)
}

fn group_assets(assets: &[ContextAssetRow]) -> HashMap<&str, Vec<&ContextAssetRow>> {
    let mut grouped: HashMap<&str, Vec<&ContextAssetRow>> = HashMap::new();
    for asset in assets {
        grouped
            .entry(asset.turn_id.as_str())
            .or_default()
            .push(asset);
    }
    for rows in grouped.values_mut() {
        rows.sort_by_key(|row| row.asset.position);
    }
    grouped
}

fn build_coverage(
    launches: &[AgentLaunchRow],
    turns: &[AgentTurnRow],
    assets: &[ContextAssetRow],
) -> ContextCoverageDto {
    let launch_by_turn = turns
        .iter()
        .map(|turn| (turn.id.as_str(), turn.launch_id.as_str()))
        .collect::<HashMap<_, _>>();
    let source_observable_agent_sessions = assets
        .iter()
        .filter(|row| {
            is_catalog_instruction_kind(row.asset.kind)
                && (row.asset.source_path.is_some()
                    || matches!(
                        row.asset.kind,
                        ContextAssetKind::SkillInstructions | ContextAssetKind::SurfaceInstructions
                    ))
        })
        .filter_map(|row| launch_by_turn.get(row.turn_id.as_str()).copied())
        .collect::<HashSet<_>>()
        .len() as u64;
    ContextCoverageDto {
        complete_launches: count_launches(launches, "complete"),
        partial_launches: count_launches(launches, "partial"),
        prompt_only_launches: count_launches(launches, "prompt_only"),
        capturing_launches: count_launches(launches, "capturing"),
        attributed_turns: count_turn_coverage(turns, "assembled"),
        provider_total_only_turns: count_turn_coverage(turns, "provider_total_only"),
        unknown_turns: count_turn_coverage(turns, "unknown"),
        prompt_artifacts_available: turns
            .iter()
            .filter(|turn| artifact_available(&turn.task_prompt_path))
            .count() as u64,
        conversations_available: launches
            .iter()
            .filter(|launch| artifact_available(&launch.conversation_path))
            .count() as u64,
        source_observable_agent_sessions,
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AgentSessionUsage {
    lifetime_input_tokens: Option<u64>,
    peak_context_percent: Option<f64>,
}

fn build_agent_session_usage(
    launches: &[AgentLaunchRow],
    turns: &[AgentTurnRow],
) -> HashMap<String, AgentSessionUsage> {
    let mut turns_by_launch: HashMap<&str, Vec<&AgentTurnRow>> = HashMap::new();
    for turn in turns {
        turns_by_launch
            .entry(turn.launch_id.as_str())
            .or_default()
            .push(turn);
    }
    launches
        .iter()
        .map(|launch| {
            let launch_turns = turns_by_launch
                .remove(launch.id.as_str())
                .unwrap_or_default();
            let input_values = launch_turns
                .iter()
                .filter_map(|turn| normalized_total_input(launch, turn))
                .collect::<Vec<_>>();
            let lifetime_input_tokens = if input_values.is_empty() {
                None
            } else if launch.provider == "codex" {
                input_values.into_iter().max()
            } else {
                Some(input_values.into_iter().sum())
            };
            let peak_context_percent = launch_turns
                .iter()
                .filter_map(|turn| {
                    let input = turn.peak_input_tokens.filter(|value| *value >= 0)? as f64;
                    let window = turn.context_window_tokens.filter(|value| *value > 0)? as f64;
                    Some(100.0 * input / window)
                })
                .max_by(f64::total_cmp);
            (
                launch.id.clone(),
                AgentSessionUsage {
                    lifetime_input_tokens,
                    peak_context_percent,
                },
            )
        })
        .collect()
}

fn normalized_total_input(launch: &AgentLaunchRow, turn: &AgentTurnRow) -> Option<u64> {
    if let Some(value) = turn.provider_total_input_tokens {
        return Some(nonnegative(value));
    }
    let input = nonnegative(turn.provider_input_tokens?);
    if launch.provider == "codex" {
        Some(input)
    } else {
        Some(
            input
                + turn.cache_read_tokens.map_or(0, nonnegative)
                + turn.cache_write_tokens.map_or(0, nonnegative),
        )
    }
}

fn build_totals(
    launches: &[AgentLaunchRow],
    turns: &[AgentTurnRow],
    assets: &[ContextAssetRow],
    usage_by_launch: &HashMap<String, AgentSessionUsage>,
) -> SessionSetTotals {
    let mut initial_prompt_values = turns
        .iter()
        .filter(|turn| turn.input_op == "initial" && turn.context_coverage == "assembled")
        .map(|turn| nonnegative(turn.supplied_context_tokens))
        .collect::<Vec<_>>();
    initial_prompt_values.sort_unstable();
    let initial_prompt_agent_sessions = turns
        .iter()
        .filter(|turn| turn.input_op == "initial" && turn.context_coverage == "assembled")
        .map(|turn| turn.launch_id.as_str())
        .collect::<HashSet<_>>()
        .len() as u64;
    let mut lifetime_input_values = usage_by_launch
        .values()
        .filter_map(|usage| usage.lifetime_input_tokens)
        .collect::<Vec<_>>();
    lifetime_input_values.sort_unstable();
    let mut peak_context_values = usage_by_launch
        .values()
        .filter_map(|usage| usage.peak_context_percent)
        .collect::<Vec<_>>();
    peak_context_values.sort_by(f64::total_cmp);
    let steered_launches = turns
        .iter()
        .filter(|turn| turn.input_op == "steer")
        .map(|turn| turn.launch_id.as_str())
        .collect::<HashSet<_>>()
        .len() as u64;
    let instruction_tokens = assets
        .iter()
        .filter(|row| is_instruction_kind(row.asset.kind))
        .map(|row| row.asset.attributed_tokens)
        .sum::<u64>();
    let attributed_tokens = assets
        .iter()
        .map(|row| row.asset.attributed_tokens)
        .sum::<u64>();
    let initial_prompt_tokens =
        (!initial_prompt_values.is_empty()).then(|| initial_prompt_values.iter().sum());
    let instruction_tokens = initial_prompt_tokens.and_then(|initial_prompt_tokens| {
        (attributed_tokens > 0 || initial_prompt_tokens == 0).then_some(instruction_tokens)
    });
    SessionSetTotals {
        runs: launches
            .iter()
            .map(|launch| launch.run_id.as_str())
            .collect::<HashSet<_>>()
            .len() as u64,
        agent_sessions: launches.len() as u64,
        turns: turns.len() as u64,
        initial_prompt_tokens,
        initial_prompt_agent_sessions,
        median_initial_prompt_tokens: percentile(&initial_prompt_values, 50),
        p95_initial_prompt_tokens: percentile(&initial_prompt_values, 95),
        instruction_tokens,
        lifetime_input_tokens: (!lifetime_input_values.is_empty())
            .then(|| lifetime_input_values.iter().sum()),
        lifetime_input_agent_sessions: lifetime_input_values.len() as u64,
        median_lifetime_input_tokens: percentile(&lifetime_input_values, 50),
        p95_lifetime_input_tokens: percentile(&lifetime_input_values, 95),
        median_peak_context_percent: percentile_f64(&peak_context_values, 50),
        p95_peak_context_percent: percentile_f64(&peak_context_values, 95),
        peak_context_agent_sessions: peak_context_values.len() as u64,
        completed_launches: count_outcomes(launches, "completed"),
        failed_launches: count_outcomes(launches, "failed"),
        interrupted_launches: count_outcomes(launches, "interrupted"),
        running_launches: count_outcomes(launches, "running"),
        steering_turns: turns.iter().filter(|turn| turn.input_op == "steer").count() as u64,
        steered_launches,
    }
}

fn build_session_lanes(
    launches: &[AgentLaunchRow],
    turns: &[AgentTurnRow],
    assets_by_turn: &HashMap<&str, Vec<&ContextAssetRow>>,
    usage_by_launch: &HashMap<String, AgentSessionUsage>,
) -> Vec<SessionLane> {
    let mut turns_by_launch: HashMap<&str, Vec<&AgentTurnRow>> = HashMap::new();
    for turn in turns {
        turns_by_launch
            .entry(turn.launch_id.as_str())
            .or_default()
            .push(turn);
    }
    launches
        .iter()
        .map(|launch| {
            let mut launch_turns = turns_by_launch
                .remove(launch.id.as_str())
                .unwrap_or_default();
            launch_turns.sort_by_key(|turn| turn.ordinal);
            let steering_turns = launch_turns
                .iter()
                .filter(|turn| turn.input_op == "steer")
                .count() as u64;
            let usage = usage_by_launch.get(&launch.id).copied().unwrap_or_default();
            SessionLane {
                id: launch.id.clone(),
                run_id: launch.run_id.clone(),
                started_at: launch.started_at,
                outcome: launch.outcome.clone(),
                steering_turns: Some(steering_turns),
                lifetime_input_tokens: usage.lifetime_input_tokens,
                peak_context_percent: usage.peak_context_percent,
                provider: launch.provider.clone(),
                model: launch.model.clone(),
                surface: launch.surface.clone(),
                wave: launch.wave.clone(),
                project: launch.project.clone(),
                task: launch.task.clone(),
                flow: launch.flow.clone(),
                skill: launch.skill.clone(),
                turns: launch_turns
                    .into_iter()
                    .map(|turn| TurnLane {
                        id: turn.id.clone(),
                        ordinal: nonnegative(turn.ordinal),
                        supplied_context_tokens: (turn.input_op == "initial"
                            && turn.context_coverage == "assembled")
                            .then(|| nonnegative(turn.supplied_context_tokens)),
                        assets: assets_by_turn
                            .get(turn.id.as_str())
                            .into_iter()
                            .flatten()
                            .map(|row| lane_asset(launch, row))
                            .collect(),
                    })
                    .collect(),
            }
        })
        .collect()
}

fn lane_asset(launch: &AgentLaunchRow, row: &ContextAssetRow) -> ContextLaneAsset {
    let canonical = canonical_identity(launch, row);
    ContextLaneAsset {
        node_id: revision_node_id(row.asset.kind, &canonical.key, &row.asset.content_sha256),
        kind: row.asset.kind,
        label: canonical.label,
        attributed_tokens: row.asset.attributed_tokens,
    }
}

#[derive(Debug, Clone, Default)]
struct FlameAccumulator {
    attributed_tokens: u64,
    runs: BTreeSet<String>,
    agent_sessions: BTreeSet<String>,
    turns: BTreeSet<String>,
}

impl FlameAccumulator {
    fn add(&mut self, run_id: &str, launch_id: &str, turn_id: &str, attributed_tokens: u64) {
        self.attributed_tokens += attributed_tokens;
        self.runs.insert(run_id.to_string());
        self.agent_sessions.insert(launch_id.to_string());
        self.turns.insert(turn_id.to_string());
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RevisionKey {
    kind: ContextAssetKind,
    canonical_key: String,
    label: String,
    source_path: Option<String>,
    content_sha256: String,
}

type SourceGroupKey = (ContextAssetKind, String, String, Option<String>);

#[derive(Debug, Clone, Default)]
struct RevisionAccumulator {
    flame: FlameAccumulator,
    precedence_layers: BTreeSet<String>,
    candidates: BTreeMap<String, EvidenceCandidate>,
}

#[derive(Debug, Clone)]
struct FlameNodeDescriptor {
    id: String,
    level: ContextFlameLevel,
    kind: Option<ContextAssetKind>,
    label: String,
    source_path: Option<String>,
    content_sha256: Option<String>,
}

#[derive(Debug, Clone)]
struct EvidenceCandidate {
    address: TraceAddress,
    started_at: i64,
    outcome: String,
    capture: String,
    provider: String,
    model: Option<String>,
    supplied_context_tokens: Option<u64>,
    selected_source_tokens: u64,
    steering_turns: Option<u64>,
    prompt_artifact_available: bool,
    conversation_available: bool,
}

fn build_revisions(
    assets: &[ContextAssetRow],
    launch_by_id: &HashMap<&str, &AgentLaunchRow>,
    turn_by_id: &HashMap<&str, &AgentTurnRow>,
    turns: &[AgentTurnRow],
) -> BTreeMap<RevisionKey, RevisionAccumulator> {
    let mut steering_by_launch: HashMap<&str, u64> = HashMap::new();
    for turn in turns.iter().filter(|turn| turn.input_op == "steer") {
        *steering_by_launch
            .entry(turn.launch_id.as_str())
            .or_default() += 1;
    }

    let mut revisions = BTreeMap::new();
    for row in assets {
        let Some(turn) = turn_by_id.get(row.turn_id.as_str()) else {
            continue;
        };
        let Some(launch) = launch_by_id.get(turn.launch_id.as_str()) else {
            continue;
        };
        let canonical = canonical_identity(launch, row);
        let revision = revisions
            .entry(RevisionKey {
                kind: row.asset.kind,
                canonical_key: canonical.key,
                label: canonical.label,
                source_path: canonical.path,
                content_sha256: row.asset.content_sha256.clone(),
            })
            .or_insert_with(RevisionAccumulator::default);
        revision.flame.add(
            &launch.run_id,
            &launch.id,
            &turn.id,
            row.asset.attributed_tokens,
        );
        revision
            .precedence_layers
            .insert(row.asset.included_by.clone());
        let candidate = revision
            .candidates
            .entry(turn.id.clone())
            .or_insert_with(|| EvidenceCandidate {
                address: TraceAddress {
                    run_id: launch.run_id.clone(),
                    launch_id: launch.id.clone(),
                    turn_id: turn.id.clone(),
                },
                started_at: turn.started_at,
                outcome: launch.outcome.clone(),
                capture: launch.capture_status.clone(),
                provider: launch.provider.clone(),
                model: launch.model.clone(),
                supplied_context_tokens: (turn.context_coverage == "assembled")
                    .then(|| nonnegative(turn.supplied_context_tokens)),
                selected_source_tokens: 0,
                steering_turns: Some(
                    steering_by_launch
                        .get(launch.id.as_str())
                        .copied()
                        .unwrap_or(0),
                ),
                prompt_artifact_available: artifact_available(&turn.task_prompt_path),
                conversation_available: artifact_available(&launch.conversation_path),
            });
        candidate.selected_source_tokens += row.asset.attributed_tokens;
    }
    revisions
}

fn build_flame(
    launches: &[AgentLaunchRow],
    turns: &[AgentTurnRow],
    revisions: &BTreeMap<RevisionKey, RevisionAccumulator>,
) -> ContextFlameNode {
    let mut by_source: BTreeMap<SourceGroupKey, Vec<(ContextFlameNode, FlameAccumulator)>> =
        BTreeMap::new();
    for (key, revision) in revisions {
        let node = node_from_accumulator(
            FlameNodeDescriptor {
                id: revision_node_id(key.kind, &key.canonical_key, &key.content_sha256),
                level: ContextFlameLevel::Revision,
                kind: Some(key.kind),
                label: short_hash(&key.content_sha256),
                source_path: key.source_path.clone(),
                content_sha256: Some(key.content_sha256.clone()),
            },
            revision.flame.clone(),
            Vec::new(),
        );
        by_source
            .entry((
                key.kind,
                key.canonical_key.clone(),
                key.label.clone(),
                key.source_path.clone(),
            ))
            .or_default()
            .push((node, revision.flame.clone()));
    }

    let mut by_kind: BTreeMap<ContextAssetKind, Vec<(ContextFlameNode, FlameAccumulator)>> =
        BTreeMap::new();
    for ((kind, canonical_key, label, source_path), mut built_children) in by_source {
        built_children.sort_by(|left, right| node_order(&left.0, &right.0));
        let accumulator =
            merge_accumulators(built_children.iter().map(|(_, accumulator)| accumulator));
        let children = built_children.into_iter().map(|(node, _)| node).collect();
        let node = node_from_accumulator(
            FlameNodeDescriptor {
                id: source_node_id(kind, &canonical_key),
                level: ContextFlameLevel::Source,
                kind: Some(kind),
                label,
                source_path,
                content_sha256: None,
            },
            accumulator.clone(),
            children,
        );
        by_kind.entry(kind).or_default().push((node, accumulator));
    }

    let mut kind_nodes = Vec::new();
    for (kind, mut built_children) in by_kind {
        built_children.sort_by(|left, right| node_order(&left.0, &right.0));
        let accumulator =
            merge_accumulators(built_children.iter().map(|(_, accumulator)| accumulator));
        let children = built_children.into_iter().map(|(node, _)| node).collect();
        kind_nodes.push(node_from_accumulator(
            FlameNodeDescriptor {
                id: stable_id(&format!("kind\0{}", kind.as_str())),
                level: ContextFlameLevel::Kind,
                kind: Some(kind),
                label: kind_label(kind.as_str()),
                source_path: None,
                content_sha256: None,
            },
            accumulator,
            children,
        ));
    }
    sort_nodes(&mut kind_nodes);
    let mut root_accumulator = FlameAccumulator {
        attributed_tokens: kind_nodes.iter().map(|node| node.attributed_tokens).sum(),
        ..FlameAccumulator::default()
    };
    root_accumulator.agent_sessions = turns.iter().map(|turn| turn.launch_id.clone()).collect();
    root_accumulator.runs = launches
        .iter()
        .filter(|launch| root_accumulator.agent_sessions.contains(&launch.id))
        .map(|launch| launch.run_id.clone())
        .collect();
    root_accumulator.turns = turns.iter().map(|turn| turn.id.clone()).collect();
    node_from_accumulator(
        FlameNodeDescriptor {
            id: "session-set".to_string(),
            level: ContextFlameLevel::SessionSet,
            kind: None,
            label: "Initial prompts".to_string(),
            source_path: None,
            content_sha256: None,
        },
        root_accumulator,
        kind_nodes,
    )
}

fn node_from_accumulator(
    descriptor: FlameNodeDescriptor,
    accumulator: FlameAccumulator,
    children: Vec<ContextFlameNode>,
) -> ContextFlameNode {
    ContextFlameNode {
        id: descriptor.id,
        level: descriptor.level,
        kind: descriptor.kind,
        label: descriptor.label,
        source_path: descriptor.source_path,
        content_sha256: descriptor.content_sha256,
        attributed_tokens: accumulator.attributed_tokens,
        run_count: accumulator.runs.len() as u64,
        agent_session_count: accumulator.agent_sessions.len() as u64,
        turn_count: accumulator.turns.len() as u64,
        children,
    }
}

fn merge_accumulators<'a>(
    accumulators: impl IntoIterator<Item = &'a FlameAccumulator>,
) -> FlameAccumulator {
    let mut accumulator = FlameAccumulator::default();
    for child in accumulators {
        accumulator.attributed_tokens += child.attributed_tokens;
        accumulator.runs.extend(child.runs.iter().cloned());
        accumulator
            .agent_sessions
            .extend(child.agent_sessions.iter().cloned());
        accumulator.turns.extend(child.turns.iter().cloned());
    }
    accumulator
}

fn sort_nodes(nodes: &mut [ContextFlameNode]) {
    nodes.sort_by(node_order);
}

fn node_order(left: &ContextFlameNode, right: &ContextFlameNode) -> std::cmp::Ordering {
    right
        .attributed_tokens
        .cmp(&left.attributed_tokens)
        .then_with(|| left.label.cmp(&right.label))
}

fn build_evidence(revisions: BTreeMap<RevisionKey, RevisionAccumulator>) -> Vec<SourceEvidence> {
    let mut evidence = revisions
        .into_iter()
        .map(|(key, revision)| {
            let candidates = revision.candidates.into_values().collect::<Vec<_>>();
            let mut per_turn = candidates
                .iter()
                .map(|candidate| candidate.selected_source_tokens)
                .collect::<Vec<_>>();
            per_turn.sort_unstable();
            let launch_candidates = candidates
                .iter()
                .map(|candidate| (candidate.address.launch_id.as_str(), candidate))
                .collect::<BTreeMap<_, _>>();
            let steering_turns = launch_candidates
                .values()
                .map(|candidate| candidate.steering_turns)
                .collect::<Option<Vec<_>>>()
                .map(|values| values.into_iter().sum());
            let mut provider_models = BTreeMap::new();
            for candidate in launch_candidates.values() {
                *provider_models
                    .entry((candidate.provider.clone(), candidate.model.clone()))
                    .or_insert(0) += 1;
            }
            let current_source_hashes = key
                .source_path
                .as_deref()
                .and_then(|path| hash_current_source(key.kind, path));
            SourceEvidence {
                node_id: revision_node_id(key.kind, &key.canonical_key, &key.content_sha256),
                label: key.label,
                kind: key.kind,
                source_path: key.source_path,
                content_sha256: key.content_sha256,
                current_content_sha256: current_source_hashes
                    .as_ref()
                    .map(|hashes| hashes.effective.clone()),
                current_source_sha256: current_source_hashes.map(|hashes| hashes.source),
                precedence_layers: revision.precedence_layers.into_iter().collect(),
                measurements: SourceMeasurements {
                    exposed_sessions: revision.flame.runs.len() as u64,
                    exposed_launches: launch_candidates.len() as u64,
                    exposed_turns: candidates.len() as u64,
                    attributed_tokens: revision.flame.attributed_tokens,
                    median_tokens_per_exposed_turn: percentile(&per_turn, 50),
                    p95_tokens_per_exposed_turn: percentile(&per_turn, 95),
                    first_seen: candidates
                        .iter()
                        .map(|candidate| candidate.started_at)
                        .min(),
                    last_seen: candidates
                        .iter()
                        .map(|candidate| candidate.started_at)
                        .max(),
                    completed_launches: launch_candidates
                        .values()
                        .filter(|candidate| candidate.outcome == "completed")
                        .count() as u64,
                    failed_launches: launch_candidates
                        .values()
                        .filter(|candidate| candidate.outcome == "failed")
                        .count() as u64,
                    steering_turns,
                    complete_capture_launches: launch_candidates
                        .values()
                        .filter(|candidate| candidate.capture == "complete")
                        .count() as u64,
                    provider_models: provider_models
                        .into_iter()
                        .map(
                            |((provider, model), exposed_launches)| ProviderModelExposure {
                                provider,
                                model,
                                exposed_launches,
                            },
                        )
                        .collect(),
                },
                representatives: select_representatives(&candidates),
            }
        })
        .collect::<Vec<_>>();
    evidence.sort_by(|left, right| {
        right
            .measurements
            .attributed_tokens
            .cmp(&left.measurements.attributed_tokens)
            .then_with(|| left.label.cmp(&right.label))
    });
    evidence
}

#[derive(Debug, Clone)]
struct InstructionSourceDraft {
    id: String,
    label: String,
    kind: ContextAssetKind,
    source_path: String,
    impressions: Option<u64>,
    observed_revision_hashes: BTreeSet<String>,
    last_seen: Option<i64>,
    current_revision_node_id: Option<String>,
    observed_revision_node_ids: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct CatalogInstructionSource {
    label: String,
    kind: ContextAssetKind,
    source_path: String,
}

fn build_instruction_sources(
    catalog_sources: &[CatalogInstructionSource],
    aggregate_root: &ContextFlameNode,
    evidence: &mut Vec<SourceEvidence>,
) -> Vec<InstructionSourceSummary> {
    let mut drafts = BTreeMap::new();
    let mut observed_sources = Vec::new();
    collect_observed_instruction_sources(aggregate_root, &mut observed_sources);

    for node in observed_sources {
        let Some(kind) = node.kind else {
            continue;
        };
        let Some(source_path) = node.source_path.clone() else {
            continue;
        };
        let observed_revision_node_ids = node
            .children
            .iter()
            .map(|child| child.id.clone())
            .collect::<BTreeSet<_>>();
        let observed_revision_hashes = node
            .children
            .iter()
            .filter_map(|child| child.content_sha256.clone())
            .collect::<BTreeSet<_>>();
        drafts.insert(
            (kind, source_path.clone()),
            InstructionSourceDraft {
                id: node.id.clone(),
                label: node.label.clone(),
                kind,
                source_path,
                impressions: Some(node.agent_session_count),
                observed_revision_hashes,
                last_seen: last_seen_for_revisions(&observed_revision_node_ids, evidence),
                current_revision_node_id: None,
                observed_revision_node_ids,
            },
        );
    }

    for source in catalog_sources {
        drafts
            .entry((source.kind, source.source_path.clone()))
            .and_modify(|draft| draft.label = source.label.clone())
            .or_insert_with(|| InstructionSourceDraft {
                id: source_node_id(source.kind, &source.source_path),
                label: source.label.clone(),
                kind: source.kind,
                source_path: source.source_path.clone(),
                impressions: (!matches!(source.kind, ContextAssetKind::RepoInstructions))
                    .then_some(0),
                observed_revision_hashes: BTreeSet::new(),
                last_seen: None,
                current_revision_node_id: None,
                observed_revision_node_ids: BTreeSet::new(),
            });
    }

    for draft in drafts.values_mut() {
        let Some(hashes) = hash_current_source(draft.kind, &draft.source_path) else {
            continue;
        };
        let current = evidence
            .iter()
            .find(|item| {
                item.kind == draft.kind
                    && item.content_sha256 == hashes.effective
                    && (item.source_path.as_deref() == Some(draft.source_path.as_str())
                        || draft.observed_revision_node_ids.contains(&item.node_id))
            })
            .cloned();
        if let Some(mut current) = current {
            if current.source_path.as_deref() == Some(draft.source_path.as_str()) {
                draft.current_revision_node_id = Some(current.node_id);
                continue;
            }
            let node_id = revision_node_id(draft.kind, &draft.source_path, &hashes.effective);
            current.node_id = node_id.clone();
            current.label = draft.label.clone();
            current.source_path = Some(draft.source_path.clone());
            current.current_content_sha256 = Some(hashes.effective);
            current.current_source_sha256 = Some(hashes.source);
            evidence.push(current);
            draft.current_revision_node_id = Some(node_id);
            continue;
        }

        let node_id = revision_node_id(draft.kind, &draft.source_path, &hashes.effective);
        evidence.push(SourceEvidence {
            node_id: node_id.clone(),
            label: draft.label.clone(),
            kind: draft.kind,
            source_path: Some(draft.source_path.clone()),
            content_sha256: hashes.effective.clone(),
            current_content_sha256: Some(hashes.effective),
            current_source_sha256: Some(hashes.source),
            precedence_layers: vec!["current file (not observed in this session set)".to_string()],
            measurements: empty_source_measurements(),
            representatives: Vec::new(),
        });
        draft.current_revision_node_id = Some(node_id);
    }

    let mut sources = drafts
        .into_values()
        .map(|draft| InstructionSourceSummary {
            id: draft.id,
            label: draft.label,
            kind: draft.kind,
            source_path: draft.source_path,
            impressions: draft.impressions,
            observed_revisions: draft.observed_revision_hashes.len() as u64,
            last_seen: draft.last_seen,
            current_revision_node_id: draft.current_revision_node_id,
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| {
        right
            .impressions
            .cmp(&left.impressions)
            .then_with(|| left.label.cmp(&right.label))
    });
    sources
}

fn empty_source_measurements() -> SourceMeasurements {
    SourceMeasurements {
        exposed_sessions: 0,
        exposed_launches: 0,
        exposed_turns: 0,
        attributed_tokens: 0,
        median_tokens_per_exposed_turn: None,
        p95_tokens_per_exposed_turn: None,
        first_seen: None,
        last_seen: None,
        completed_launches: 0,
        failed_launches: 0,
        steering_turns: Some(0),
        complete_capture_launches: 0,
        provider_models: Vec::new(),
    }
}

fn collect_observed_instruction_sources<'a>(
    node: &'a ContextFlameNode,
    sources: &mut Vec<&'a ContextFlameNode>,
) {
    if node.level == ContextFlameLevel::Source && node.kind.is_some_and(is_catalog_instruction_kind)
    {
        sources.push(node);
    }
    for child in &node.children {
        collect_observed_instruction_sources(child, sources);
    }
}

fn last_seen_for_revisions(
    revision_ids: &BTreeSet<String>,
    evidence: &[SourceEvidence],
) -> Option<i64> {
    evidence
        .iter()
        .filter(|item| revision_ids.contains(&item.node_id))
        .filter_map(|item| item.measurements.last_seen)
        .max()
}

fn instruction_labels_match(kind: ContextAssetKind, observed: &str, catalog: &str) -> bool {
    let normalize = |label: &str| {
        label
            .trim_end_matches(".md")
            .trim_end_matches(" instructions")
            .to_ascii_lowercase()
    };
    if kind == ContextAssetKind::SurfaceInstructions {
        return normalize(observed).trim_end_matches(" surface")
            == normalize(catalog).trim_end_matches(" surface");
    }
    normalize(observed) == normalize(catalog)
}

fn attach_catalog_source_paths(
    assets: &mut [ContextAssetRow],
    catalog_sources: &[CatalogInstructionSource],
) {
    for row in assets.iter_mut().filter(|row| {
        row.asset.source_path.is_none() && is_catalog_instruction_kind(row.asset.kind)
    }) {
        let mut matches = catalog_sources.iter().filter(|source| {
            source.kind == row.asset.kind
                && instruction_labels_match(row.asset.kind, &row.asset.label, &source.label)
        });
        let Some(source) = matches.next() else {
            continue;
        };
        if matches.next().is_none() {
            row.asset.source_path = Some(source.source_path.clone());
        }
    }
}

fn discover_instruction_sources(query: &SessionSetQuery) -> Vec<CatalogInstructionSource> {
    let mut sources = BTreeMap::new();
    for repo_path in &query.repo_paths {
        let repo =
            fs::canonicalize(repo_path).unwrap_or_else(|_| normalize_path(Path::new(repo_path)));
        for name in ["AGENTS.md", "CLAUDE.md"] {
            add_catalog_source(
                &mut sources,
                ContextAssetKind::RepoInstructions,
                name.to_string(),
                repo.join(name),
            );
        }

        let builtins = repo.join("rust/loopflow/src/engine/builtins");
        add_catalog_source(
            &mut sources,
            ContextAssetKind::OperatingInstructions,
            "LOOPFLOW.md".to_string(),
            builtins.join("LOOPFLOW.md"),
        );
        add_markdown_catalog(
            &mut sources,
            ContextAssetKind::SurfaceInstructions,
            &builtins.join("surfaces"),
            false,
            |relative| format!("{} surface", relative.trim_end_matches(".md")),
        );

        // Keep one effective in-repo file for each skill name. Repo agent skills
        // are fallback-only, builtins shadow those, and repo loopflow skills win.
        let mut effective_skills = BTreeMap::new();
        overlay_skill_sources(
            &mut effective_skills,
            collect_markdown_catalog(
                ContextAssetKind::SkillInstructions,
                &repo.join(".agents/skills"),
                true,
                |relative| {
                    Path::new(relative)
                        .parent()
                        .map(|path| path.to_string_lossy().to_string())
                        .filter(|label| !label.is_empty())
                        .unwrap_or_else(|| "SKILL".to_string())
                },
            ),
        );
        if let Ok(categories) = fs::read_dir(&builtins) {
            for category in categories.flatten() {
                let category_name = category.file_name().to_string_lossy().to_string();
                let skill_root = category.path().join("skill");
                overlay_skill_sources(
                    &mut effective_skills,
                    collect_markdown_catalog(
                        ContextAssetKind::SkillInstructions,
                        &skill_root,
                        false,
                        |relative| {
                            let name = relative.trim_end_matches(".md");
                            if category_name == "gstack" {
                                format!("gstack/{name}")
                            } else {
                                name.to_string()
                            }
                        },
                    ),
                );
            }
        }

        for root in [repo.join(".claude/commands"), repo.join(".lf/skills")] {
            overlay_skill_sources(
                &mut effective_skills,
                collect_markdown_catalog(
                    ContextAssetKind::SkillInstructions,
                    &root,
                    false,
                    |relative| relative.trim_end_matches(".md").to_string(),
                ),
            );
        }
        for source in effective_skills.into_values() {
            sources.insert((source.kind, source.source_path.clone()), source);
        }
    }
    sources.into_values().collect()
}

fn collect_markdown_catalog(
    kind: ContextAssetKind,
    root: &Path,
    skill_manifests_only: bool,
    label: impl Fn(&str) -> String,
) -> Vec<CatalogInstructionSource> {
    let mut sources = BTreeMap::new();
    add_markdown_catalog(&mut sources, kind, root, skill_manifests_only, label);
    sources.into_values().collect()
}

fn overlay_skill_sources(
    effective: &mut BTreeMap<String, CatalogInstructionSource>,
    sources: Vec<CatalogInstructionSource>,
) {
    for source in sources {
        effective.insert(source.label.clone(), source);
    }
}

fn add_markdown_catalog(
    sources: &mut BTreeMap<(ContextAssetKind, String), CatalogInstructionSource>,
    kind: ContextAssetKind,
    root: &Path,
    skill_manifests_only: bool,
    label: impl Fn(&str) -> String,
) {
    for path in markdown_files(root) {
        if skill_manifests_only && path.file_name().is_none_or(|name| name != "SKILL.md") {
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        add_catalog_source(sources, kind, label(&relative.to_string_lossy()), path);
    }
}

fn markdown_files(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            paths.extend(markdown_files(&path));
        } else if file_type.is_file() && path.extension().is_some_and(|extension| extension == "md")
        {
            paths.push(path);
        }
    }
    paths.sort();
    paths
}

fn add_catalog_source(
    sources: &mut BTreeMap<(ContextAssetKind, String), CatalogInstructionSource>,
    kind: ContextAssetKind,
    label: String,
    path: PathBuf,
) {
    if !path.is_file() {
        return;
    }
    let source_path = fs::canonicalize(&path)
        .unwrap_or_else(|_| normalize_path(&path))
        .to_string_lossy()
        .to_string();
    sources.insert(
        (kind, source_path.clone()),
        CatalogInstructionSource {
            label,
            kind,
            source_path,
        },
    );
}

fn select_representatives(candidates: &[EvidenceCandidate]) -> Vec<RepresentativeTrace> {
    // Roles are surfaced in priority order and each is a distinct session. When
    // the best candidate for a later role already represents an earlier role,
    // take the next-best session rather than repeating one or dropping a
    // role that the population can still fill.
    let mut selected = Vec::new();
    let mut claimed_sessions = HashSet::new();
    if let Some(candidate) = candidates
        .iter()
        .filter(|candidate| {
            candidate.outcome == "completed"
                && candidate.capture == "complete"
                && candidate.steering_turns.unwrap_or(0) == 0
        })
        .min_by_key(|candidate| candidate.supplied_context_tokens.unwrap_or(u64::MAX))
    {
        claimed_sessions.insert(candidate.address.run_id.clone());
        selected.push(representative(EvidenceRole::SmoothComplete, candidate));
    }
    if let Some(candidate) = candidates
        .iter()
        .filter(|candidate| !claimed_sessions.contains(&candidate.address.run_id))
        .filter(|candidate| candidate.outcome == "completed")
        .max_by_key(|candidate| candidate.supplied_context_tokens.unwrap_or(0))
    {
        claimed_sessions.insert(candidate.address.run_id.clone());
        selected.push(representative(EvidenceRole::HighContextComplete, candidate));
    }
    if let Some(candidate) = candidates
        .iter()
        .filter(|candidate| !claimed_sessions.contains(&candidate.address.run_id))
        .filter(|candidate| {
            candidate.outcome == "failed"
                || candidate.outcome == "interrupted"
                || candidate.steering_turns.unwrap_or(0) > 0
        })
        .max_by_key(|candidate| candidate.selected_source_tokens)
    {
        claimed_sessions.insert(candidate.address.run_id.clone());
        selected.push(representative(EvidenceRole::FailedOrSteered, candidate));
    }
    if let Some(candidate) = candidates
        .iter()
        .filter(|candidate| !claimed_sessions.contains(&candidate.address.run_id))
        .max_by_key(|candidate| candidate.started_at)
    {
        selected.push(representative(EvidenceRole::Recent, candidate));
    }
    selected
}

fn representative(role: EvidenceRole, candidate: &EvidenceCandidate) -> RepresentativeTrace {
    RepresentativeTrace {
        role,
        address: candidate.address.clone(),
        outcome: candidate.outcome.clone(),
        supplied_context_tokens: candidate.supplied_context_tokens,
        selected_source_tokens: candidate.selected_source_tokens,
        prompt_artifact_available: candidate.prompt_artifact_available,
        conversation_available: candidate.conversation_available,
    }
}

#[derive(Debug, Clone)]
struct CanonicalIdentity {
    key: String,
    label: String,
    path: Option<String>,
}

fn canonical_identity(launch: &AgentLaunchRow, row: &ContextAssetRow) -> CanonicalIdentity {
    if let Some(source_path) = &row.asset.source_path {
        let path = canonical_source_path(launch, source_path);
        let label = Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| row.asset.label.clone());
        return CanonicalIdentity {
            key: path.clone(),
            label,
            path: Some(path),
        };
    }
    let key = format!("@{}/{}", row.asset.kind.as_str(), row.asset.label);
    CanonicalIdentity {
        key,
        label: context_source_label(row.asset.kind, &row.asset.label),
        path: None,
    }
}

fn context_source_label(kind: ContextAssetKind, label: &str) -> String {
    if kind != ContextAssetKind::Assembly {
        return label.to_string();
    }
    match label {
        "prompt assembly" => "unattributed prompt remainder".to_string(),
        "task prompt" => "unattributed task prompt".to_string(),
        "system prompt" => "unattributed system prompt".to_string(),
        _ => label.to_string(),
    }
}

fn canonical_source_path(launch: &AgentLaunchRow, source_path: &str) -> String {
    let source = Path::new(source_path);
    let joined = if source.is_absolute() {
        source
            .strip_prefix(&launch.worktree)
            .map(|relative| Path::new(&launch.repo).join(relative))
            .unwrap_or_else(|_| source.to_path_buf())
    } else {
        Path::new(&launch.repo).join(source)
    };
    fs::canonicalize(&joined)
        .unwrap_or_else(|_| normalize_path(&joined))
        .to_string_lossy()
        .to_string()
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn revision_node_id(kind: ContextAssetKind, canonical_key: &str, content_sha256: &str) -> String {
    stable_id(&format!(
        "revision\0{kind}\0{}\0{content_sha256}",
        canonical_key,
        kind = kind.as_str(),
    ))
}

fn source_node_id(kind: ContextAssetKind, canonical_key: &str) -> String {
    stable_id(&format!("source\0{}\0{canonical_key}", kind.as_str()))
}

fn stable_id(value: &str) -> String {
    format!("context-{}", hex::encode(Sha256::digest(value.as_bytes())))
}

fn short_hash(value: &str) -> String {
    value.chars().take(10).collect()
}

#[derive(Debug, Clone)]
struct CurrentSourceHashes {
    effective: String,
    source: String,
}

fn hash_current_source(kind: ContextAssetKind, path: &str) -> Option<CurrentSourceHashes> {
    let bytes = fs::read(path).ok()?;
    let source = hex::encode(Sha256::digest(&bytes));
    let content = String::from_utf8(bytes).ok()?;
    let effective = match kind {
        ContextAssetKind::OperatingInstructions => {
            format!("<lf:loopflow>\n{content}\n</lf:loopflow>")
        }
        ContextAssetKind::SkillInstructions => {
            crate::engine::flow::split_frontmatter(&content).map_or(content, |(_, body)| body)
        }
        // These assets combine the file with generated or journal-backed state;
        // one file hash cannot prove that the effective slice is still current.
        ContextAssetKind::Goal | ContextAssetKind::Memory => return None,
        _ => content,
    };
    Some(CurrentSourceHashes {
        effective: hex::encode(Sha256::digest(effective.as_bytes())),
        source,
    })
}

fn artifact_available(path: &str) -> bool {
    crate::trace::resolve_artifact(path).is_ok_and(|path| path.is_file())
}

fn nonnegative(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn count_launches(launches: &[AgentLaunchRow], capture: &str) -> u64 {
    launches
        .iter()
        .filter(|launch| launch.capture_status == capture)
        .count() as u64
}

fn count_outcomes(launches: &[AgentLaunchRow], outcome: &str) -> u64 {
    launches
        .iter()
        .filter(|launch| launch.outcome == outcome)
        .count() as u64
}

fn count_turn_coverage(turns: &[AgentTurnRow], coverage: &str) -> u64 {
    turns
        .iter()
        .filter(|turn| turn.context_coverage == coverage)
        .count() as u64
}

fn is_instruction_kind(kind: ContextAssetKind) -> bool {
    matches!(
        kind,
        ContextAssetKind::OperatingInstructions
            | ContextAssetKind::SurfaceInstructions
            | ContextAssetKind::ProviderInstructions
            | ContextAssetKind::RepoInstructions
            | ContextAssetKind::SkillInstructions
            | ContextAssetKind::Direction
            | ContextAssetKind::Goal
            | ContextAssetKind::Memory
    )
}

fn is_catalog_instruction_kind(kind: ContextAssetKind) -> bool {
    matches!(
        kind,
        ContextAssetKind::OperatingInstructions
            | ContextAssetKind::SurfaceInstructions
            | ContextAssetKind::ProviderInstructions
            | ContextAssetKind::RepoInstructions
            | ContextAssetKind::SkillInstructions
    )
}

fn kind_label(kind: &str) -> String {
    if kind == ContextAssetKind::Assembly.as_str() {
        return "Unattributed".to_string();
    }
    kind.split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().collect::<String>() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn percentile(sorted: &[u64], percent: usize) -> Option<u64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (sorted.len() * percent).div_ceil(100).max(1);
    Some(sorted[rank - 1])
}

fn percentile_f64(sorted: &[f64], percent: usize) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (sorted.len() * percent).div_ceil(100).max(1);
    Some(sorted[rank - 1])
}

fn print_human(snapshot: &ContextLabSnapshot) {
    if snapshot.totals.agent_sessions == 0 {
        println!("No captured agent context in the selected population.");
        return;
    }
    let totals = &snapshot.totals;
    println!(
        "POPULATION   {} runs  {} agent sessions  {} turns",
        totals.runs, totals.agent_sessions, totals.turns
    );
    println!(
        "INITIAL      {} tokens / {} agent sessions  median {}  p95 {}",
        display_optional(totals.initial_prompt_tokens),
        totals.initial_prompt_agent_sessions,
        display_optional(totals.median_initial_prompt_tokens),
        display_optional(totals.p95_initial_prompt_tokens),
    );
    println!(
        "LIFETIME     {} input tokens / {} agent sessions  peak window median {}  p95 {}",
        display_optional(totals.lifetime_input_tokens),
        totals.lifetime_input_agent_sessions,
        display_percent(totals.median_peak_context_percent),
        display_percent(totals.p95_peak_context_percent),
    );
    println!(
        "CAPTURE      {} complete  {} partial  {} prompt-only  prompts {}/{}  conversations {}/{}",
        snapshot.coverage.complete_launches,
        snapshot.coverage.partial_launches,
        snapshot.coverage.prompt_only_launches,
        snapshot.coverage.prompt_artifacts_available,
        totals.turns,
        snapshot.coverage.conversations_available,
        totals.agent_sessions,
    );
    println!("\nCONTEXT FLAME");
    for node in &snapshot.aggregate_root.children {
        println!(
            "  {:<24} {:>10} tokens  {:>5} impressions  {:>5} turns",
            node.label, node.attributed_tokens, node.agent_session_count, node.turn_count
        );
    }
}

fn display_optional(value: Option<u64>) -> String {
    value.map_or_else(|| "missing".to_string(), |value| value.to_string())
}

fn display_percent(value: Option<f64>) -> String {
    value.map_or_else(|| "missing".to_string(), |value| format!("{value:.1}%"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{ContextAsset, ContextAssetKind, ContextChannel, ContextScope};

    #[test]
    fn aggregate_preserves_missing_values_and_flame_widths() {
        let query = query();
        let launches = vec![launch("launch-a", "run-a", "completed", "complete", 100)];
        let turns = vec![
            turn("turn-a", "launch-a", 1, "assembled", 100, Some(0.25)),
            turn("turn-b", "launch-a", 2, "unknown", 0, None),
        ];
        let assets = vec![
            asset(
                "turn-a",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                "a",
                60,
            ),
            asset(
                "turn-a",
                1,
                ContextAssetKind::SkillInstructions,
                "implement",
                None,
                "b",
                40,
            ),
            asset(
                "turn-b",
                0,
                ContextAssetKind::UserMessage,
                "steering message",
                None,
                "c",
                7,
            ),
        ];

        let snapshot = aggregate(query, launches, turns, assets);

        assert_eq!(snapshot.totals.initial_prompt_tokens, Some(100));
        assert_eq!(snapshot.coverage.attributed_turns, 1);
        assert_eq!(snapshot.coverage.unknown_turns, 1);
        assert_eq!(snapshot.aggregate_root.attributed_tokens, 100);
        assert_eq!(
            snapshot
                .aggregate_root
                .children
                .iter()
                .map(|node| node.attributed_tokens)
                .sum::<u64>(),
            snapshot.aggregate_root.attributed_tokens
        );
        assert_eq!(
            snapshot.sessions[0].turns[0]
                .assets
                .iter()
                .map(|asset| asset.label.as_str())
                .collect::<Vec<_>>(),
            ["AGENTS.md", "implement"]
        );
        assert_eq!(snapshot.sessions[0].turns[1].supplied_context_tokens, None);
        assert!(snapshot.sessions[0].turns[1].assets.is_empty());
    }

    #[test]
    fn assembly_capture_is_presented_as_unattributed() {
        let snapshot = aggregate(
            query(),
            vec![launch("launch-a", "run-a", "completed", "complete", 10)],
            vec![turn("turn-a", "launch-a", 1, "assembled", 10, None)],
            vec![asset(
                "turn-a",
                0,
                ContextAssetKind::Assembly,
                "prompt assembly",
                None,
                "remainder-hash",
                10,
            )],
        );

        let kind = &snapshot.aggregate_root.children[0];
        assert_eq!(kind.label, "Unattributed");
        assert_eq!(kind.children[0].label, "unattributed prompt remainder");
        assert_eq!(
            snapshot.sessions[0].turns[0].assets[0].label,
            "unattributed prompt remainder"
        );
    }

    #[test]
    fn revisions_stay_separate_under_one_canonical_source() {
        let query = query();
        let launches = vec![
            launch("launch-a", "run-a", "completed", "complete", 100),
            launch("launch-b", "run-b", "failed", "partial", 200),
        ];
        let turns = vec![
            turn("turn-a", "launch-a", 1, "assembled", 60, Some(0.1)),
            turn("turn-b", "launch-b", 1, "assembled", 90, Some(0.2)),
        ];
        let assets = vec![
            asset(
                "turn-a",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                "old-hash",
                60,
            ),
            asset(
                "turn-b",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                "new-hash",
                90,
            ),
        ];

        let snapshot = aggregate(query, launches, turns, assets);
        let kind = &snapshot.aggregate_root.children[0];
        assert_eq!(kind.children.len(), 1);
        assert_eq!(kind.children[0].children.len(), 2);
        assert_eq!(kind.children[0].attributed_tokens, 150);
        assert_eq!(kind.children[0].run_count, 2);
        assert_eq!(kind.children[0].agent_session_count, 2);
        assert_eq!(kind.children[0].turn_count, 2);
        assert_ne!(
            kind.children[0].children[0].id,
            kind.children[0].children[1].id
        );
        assert_eq!(snapshot.evidence.len(), 2);
        assert!(snapshot.evidence.iter().any(|item| item
            .representatives
            .iter()
            .any(|trace| trace.role == EvidenceRole::FailedOrSteered)));
        let failed = snapshot
            .evidence
            .iter()
            .find(|item| item.content_sha256 == "new-hash")
            .expect("new revision evidence");
        assert_eq!(failed.measurements.first_seen, Some(101));
        assert_eq!(failed.measurements.failed_launches, 1);
        assert_eq!(failed.measurements.complete_capture_launches, 0);
    }

    #[test]
    fn smooth_representative_excludes_steered_launches() {
        let launches = vec![
            launch("launch-steered", "run-a", "completed", "complete", 100),
            launch("launch-smooth", "run-b", "completed", "complete", 200),
        ];
        let mut steered_turn = turn("turn-steer", "launch-steered", 2, "assembled", 20, None);
        steered_turn.input_op = "steer".to_string();
        let turns = vec![
            turn("turn-initial", "launch-steered", 1, "assembled", 10, None),
            steered_turn,
            turn("turn-smooth", "launch-smooth", 1, "assembled", 30, None),
        ];
        let assets = vec![
            asset(
                "turn-initial",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                "hash",
                10,
            ),
            asset(
                "turn-steer",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                "hash",
                20,
            ),
            asset(
                "turn-smooth",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                "hash",
                30,
            ),
        ];

        let snapshot = aggregate(query(), launches, turns, assets);
        let smooth = snapshot.evidence[0]
            .representatives
            .iter()
            .find(|representative| representative.role == EvidenceRole::SmoothComplete)
            .expect("smooth complete representative");

        assert_eq!(smooth.address.launch_id, "launch-smooth");
    }

    #[test]
    fn steered_only_filters_the_whole_atomic_population() {
        let launches = vec![
            launch("launch-smooth", "run-a", "completed", "complete", 100),
            launch("launch-steered", "run-b", "completed", "complete", 200),
        ];
        let mut steered = turn("turn-steered", "launch-steered", 1, "assembled", 20, None);
        steered.input_op = "steer".to_string();
        let turns = vec![
            turn("turn-smooth", "launch-smooth", 1, "assembled", 10, None),
            steered,
        ];
        let mut selection = query();
        selection.steered_only = true;

        let snapshot = aggregate(selection, launches, turns, Vec::new());

        assert_eq!(snapshot.totals.runs, 1);
        assert_eq!(snapshot.totals.agent_sessions, 1);
        assert_eq!(snapshot.totals.turns, 1);
        assert_eq!(snapshot.totals.steered_launches, 1);
        assert_eq!(snapshot.sessions[0].id, "launch-steered");
    }

    #[test]
    fn current_revision_only_keeps_launches_with_a_current_file_instruction() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("AGENTS.md");
        fs::write(&path, "Current instructions\n").unwrap();
        let current_hash = hex::encode(Sha256::digest(b"Current instructions\n"));
        let mut current_launch = launch("launch-current", "run-a", "completed", "complete", 100);
        current_launch.repo = directory.path().to_string_lossy().to_string();
        current_launch.worktree = current_launch.repo.clone();
        let mut historical_launch =
            launch("launch-historical", "run-b", "completed", "complete", 200);
        historical_launch.repo = current_launch.repo.clone();
        historical_launch.worktree = current_launch.worktree.clone();
        let mut unresolved_launch =
            launch("launch-unresolved", "run-c", "completed", "complete", 300);
        unresolved_launch.repo = current_launch.repo.clone();
        unresolved_launch.worktree = current_launch.worktree.clone();
        let turns = vec![
            turn("turn-current", "launch-current", 1, "assembled", 20, None),
            turn("turn-earlier", "launch-current", 2, "assembled", 5, None),
            turn(
                "turn-historical",
                "launch-historical",
                1,
                "assembled",
                30,
                None,
            ),
            turn(
                "turn-unresolved",
                "launch-unresolved",
                1,
                "assembled",
                10,
                None,
            ),
        ];
        let assets = vec![
            asset(
                "turn-current",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                &current_hash,
                20,
            ),
            asset(
                "turn-earlier",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                "historical-hash",
                5,
            ),
            asset(
                "turn-historical",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                Some("AGENTS.md"),
                "historical-hash",
                30,
            ),
            asset(
                "turn-unresolved",
                0,
                ContextAssetKind::RepoInstructions,
                "injected instructions",
                None,
                &current_hash,
                10,
            ),
        ];
        let mut selection = query();
        selection.current_revision_only = true;

        let snapshot = aggregate(
            selection,
            vec![current_launch, historical_launch, unresolved_launch],
            turns,
            assets,
        );

        assert_eq!(snapshot.totals.agent_sessions, 1);
        assert_eq!(snapshot.totals.turns, 2);
        assert_eq!(snapshot.sessions[0].id, "launch-current");
        assert_eq!(snapshot.aggregate_root.attributed_tokens, 20);
        assert!(snapshot
            .evidence
            .iter()
            .any(|evidence| evidence.content_sha256 == current_hash));
        assert!(snapshot
            .evidence
            .iter()
            .all(|evidence| evidence.content_sha256 != "historical-hash"));
    }

    #[test]
    fn revision_measurements_include_provider_model_mix_and_observation_window() {
        let mut codex = launch("launch-codex", "run-a", "completed", "complete", 100);
        codex.model = Some("gpt-5".to_string());
        let mut claude = launch("launch-claude", "run-b", "completed", "complete", 300);
        claude.provider = "claude".to_string();
        claude.model = None;
        let mut claude_turn = turn("turn-claude", "launch-claude", 1, "assembled", 20, None);
        claude_turn.started_at = 301;
        let turns = vec![
            turn("turn-codex", "launch-codex", 1, "assembled", 10, None),
            claude_turn,
        ];
        let assets = vec![
            asset(
                "turn-codex",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                None,
                "hash",
                10,
            ),
            asset(
                "turn-claude",
                0,
                ContextAssetKind::RepoInstructions,
                "AGENTS.md",
                None,
                "hash",
                20,
            ),
        ];

        let snapshot = aggregate(query(), vec![codex, claude], turns, assets);
        let measurements = &snapshot.evidence[0].measurements;

        assert_eq!(measurements.first_seen, Some(101));
        assert_eq!(measurements.last_seen, Some(301));
        assert_eq!(
            measurements.provider_models,
            [
                ProviderModelExposure {
                    provider: "claude".to_string(),
                    model: None,
                    exposed_launches: 1,
                },
                ProviderModelExposure {
                    provider: "codex".to_string(),
                    model: Some("gpt-5".to_string()),
                    exposed_launches: 1,
                },
            ]
        );
    }

    #[test]
    fn representatives_never_repeat_one_session_across_roles() {
        let launches = vec![launch("launch-solo", "run-a", "completed", "complete", 100)];
        let turns = vec![turn("turn-solo", "launch-solo", 1, "assembled", 30, None)];
        let assets = vec![asset(
            "turn-solo",
            0,
            ContextAssetKind::RepoInstructions,
            "AGENTS.md",
            Some("AGENTS.md"),
            "hash",
            30,
        )];

        let snapshot = aggregate(query(), launches, turns, assets);
        let representatives = &snapshot.evidence[0].representatives;

        // The lone completed, complete-capture, zero-steer run is simultaneously
        // the smooth, high-context, and recent candidate. It must surface once.
        assert_eq!(representatives.len(), 1);
        assert_eq!(representatives[0].role, EvidenceRole::SmoothComplete);
        assert_eq!(representatives[0].address.launch_id, "launch-solo");
    }

    #[test]
    fn representatives_fill_roles_from_distinct_sessions_when_possible() {
        let launches = vec![
            launch("launch-low", "run-a", "completed", "complete", 100),
            launch("launch-same-session", "run-a", "completed", "complete", 150),
            launch("launch-high", "run-b", "completed", "complete", 200),
            launch("launch-recent", "run-c", "completed", "partial", 300),
        ];
        let turns = vec![
            turn("turn-low", "launch-low", 1, "assembled", 10, None),
            turn(
                "turn-same-session",
                "launch-same-session",
                1,
                "assembled",
                100,
                None,
            ),
            turn("turn-high", "launch-high", 1, "assembled", 80, None),
            turn("turn-recent", "launch-recent", 1, "assembled", 40, None),
        ];
        let assets = turns
            .iter()
            .map(|turn| {
                asset(
                    &turn.id,
                    0,
                    ContextAssetKind::RepoInstructions,
                    "AGENTS.md",
                    Some("AGENTS.md"),
                    "hash",
                    nonnegative(turn.supplied_context_tokens),
                )
            })
            .collect();

        let snapshot = aggregate(query(), launches, turns, assets);
        let representatives = &snapshot.evidence[0].representatives;

        assert_eq!(
            representatives
                .iter()
                .map(|representative| (
                    representative.role,
                    representative.address.launch_id.as_str(),
                ))
                .collect::<Vec<_>>(),
            [
                (EvidenceRole::SmoothComplete, "launch-low"),
                (EvidenceRole::HighContextComplete, "launch-high"),
                (EvidenceRole::Recent, "launch-recent"),
            ]
        );
    }

    #[test]
    fn percentiles_count_each_captured_turn_once() {
        assert_eq!(percentile(&[10, 20, 30, 40], 50), Some(20));
        assert_eq!(percentile(&[10, 20, 30, 40], 95), Some(40));
        assert_eq!(percentile(&[], 95), None);
    }

    #[test]
    fn lifetime_input_and_peak_pressure_are_agent_session_metrics() {
        let mut codex_initial = turn("codex-1", "launch-codex", 1, "assembled", 10, None);
        codex_initial.provider_total_input_tokens = Some(100);
        codex_initial.peak_input_tokens = Some(50);
        codex_initial.context_window_tokens = Some(100);
        let mut codex_steer = turn("codex-2", "launch-codex", 2, "provider_total_only", 1, None);
        codex_steer.provider_total_input_tokens = Some(300);
        codex_steer.peak_input_tokens = Some(80);
        codex_steer.context_window_tokens = Some(100);

        let mut claude_initial = turn("claude-1", "launch-claude", 1, "assembled", 20, None);
        claude_initial.provider_total_input_tokens = Some(100);
        claude_initial.peak_input_tokens = Some(50);
        claude_initial.context_window_tokens = Some(200);
        let mut claude_steer = turn(
            "claude-2",
            "launch-claude",
            2,
            "provider_total_only",
            1,
            None,
        );
        claude_steer.provider_total_input_tokens = Some(200);
        claude_steer.peak_input_tokens = Some(90);
        claude_steer.context_window_tokens = Some(200);

        let mut claude = launch("launch-claude", "run-b", "completed", "complete", 200);
        claude.provider = "claude".to_string();
        let snapshot = aggregate(
            query(),
            vec![
                launch("launch-codex", "run-a", "completed", "complete", 100),
                claude,
            ],
            vec![codex_initial, codex_steer, claude_initial, claude_steer],
            Vec::new(),
        );

        assert_eq!(snapshot.totals.initial_prompt_tokens, Some(30));
        assert_eq!(snapshot.totals.initial_prompt_agent_sessions, 2);
        assert_eq!(snapshot.totals.lifetime_input_tokens, Some(600));
        assert_eq!(snapshot.totals.median_lifetime_input_tokens, Some(300));
        assert_eq!(snapshot.totals.p95_lifetime_input_tokens, Some(300));
        assert_eq!(snapshot.totals.median_peak_context_percent, Some(45.0));
        assert_eq!(snapshot.totals.p95_peak_context_percent, Some(80.0));
        assert_eq!(snapshot.totals.peak_context_agent_sessions, 2);
    }

    #[test]
    fn source_impressions_count_each_agent_session_once() {
        let launch = launch("launch-a", "run-a", "completed", "complete", 100);
        let turn = turn("turn-a", "launch-a", 1, "assembled", 20, None);
        let assets = vec![
            asset(
                "turn-a",
                0,
                ContextAssetKind::SkillInstructions,
                "refine",
                Some("refine.md"),
                "hash",
                10,
            ),
            asset(
                "turn-a",
                1,
                ContextAssetKind::SkillInstructions,
                "refine",
                Some("refine.md"),
                "hash",
                10,
            ),
        ];

        let snapshot = aggregate(query(), vec![launch], vec![turn], assets);
        let source = &snapshot.aggregate_root.children[0].children[0];

        assert_eq!(source.attributed_tokens, 20);
        assert_eq!(source.agent_session_count, 1);
        assert_eq!(source.run_count, 1);
        assert_eq!(snapshot.coverage.source_observable_agent_sessions, 1);
    }

    #[test]
    fn source_catalog_keeps_unseen_skills_editable_at_zero_impressions() {
        let directory = tempfile::tempdir().unwrap();
        let skills = directory.path().join(".lf/skills");
        fs::create_dir_all(&skills).unwrap();
        let seen_path = skills.join("seen.md");
        let unseen_path = skills.join("unseen.md");
        fs::write(&seen_path, "Seen instructions\n").unwrap();
        fs::write(&unseen_path, "Unseen instructions\n").unwrap();

        let repo = directory.path().canonicalize().unwrap();
        let mut selection = query();
        selection.repo_paths = vec![repo.to_string_lossy().to_string()];
        let mut agent_session = launch("launch-a", "run-a", "completed", "complete", 100);
        agent_session.repo = repo.to_string_lossy().to_string();
        agent_session.worktree = agent_session.repo.clone();
        let seen_hash = hash_current_source(
            ContextAssetKind::SkillInstructions,
            seen_path.to_str().unwrap(),
        )
        .unwrap()
        .effective;

        let snapshot = aggregate(
            selection,
            vec![agent_session],
            vec![turn("turn-a", "launch-a", 1, "assembled", 20, None)],
            vec![asset(
                "turn-a",
                0,
                ContextAssetKind::SkillInstructions,
                "seen",
                Some(".lf/skills/seen.md"),
                &seen_hash,
                20,
            )],
        );

        assert_eq!(snapshot.sources.len(), 2);
        let seen = snapshot
            .sources
            .iter()
            .find(|source| source.label == "seen")
            .unwrap();
        assert_eq!(seen.impressions, Some(1));
        assert_eq!(seen.observed_revisions, 1);
        let unseen = snapshot
            .sources
            .iter()
            .find(|source| source.label == "unseen")
            .unwrap();
        assert_eq!(unseen.impressions, Some(0));
        assert_eq!(unseen.observed_revisions, 0);
        let current_node_id = unseen.current_revision_node_id.as_ref().unwrap();
        let current = snapshot
            .evidence
            .iter()
            .find(|item| &item.node_id == current_node_id)
            .unwrap();
        assert_eq!(
            current.current_content_sha256.as_deref(),
            Some(current.content_sha256.as_str())
        );
        assert_eq!(current.measurements.exposed_launches, 0);
        assert!(current.representatives.is_empty());
    }

    #[test]
    fn source_catalog_excludes_shadowed_skill_files() {
        let directory = tempfile::tempdir().unwrap();
        let builtin = directory
            .path()
            .join("rust/loopflow/src/engine/builtins/build/skill/compress.md");
        let local = directory.path().join(".lf/skills/compress.md");
        fs::create_dir_all(builtin.parent().unwrap()).unwrap();
        fs::create_dir_all(local.parent().unwrap()).unwrap();
        fs::write(&builtin, "Builtin instructions\n").unwrap();
        fs::write(&local, "Local instructions\n").unwrap();

        let mut selection = query();
        selection.repo_paths = vec![directory.path().to_string_lossy().to_string()];
        let snapshot = aggregate(selection, Vec::new(), Vec::new(), Vec::new());

        assert_eq!(snapshot.sources.len(), 1);
        assert_eq!(snapshot.sources[0].label, "compress");
        assert_eq!(
            snapshot.sources[0].source_path,
            local.canonicalize().unwrap().to_string_lossy()
        );
    }

    #[test]
    fn source_catalog_does_not_report_uncaptured_repo_instructions_as_zero() {
        let directory = tempfile::tempdir().unwrap();
        let agents = directory.path().join("AGENTS.md");
        fs::write(&agents, "Repository instructions\n").unwrap();

        let mut selection = query();
        selection.repo_paths = vec![directory.path().to_string_lossy().to_string()];
        let snapshot = aggregate(selection, Vec::new(), Vec::new(), Vec::new());

        assert_eq!(snapshot.sources.len(), 1);
        assert_eq!(snapshot.sources[0].label, "AGENTS.md");
        assert_eq!(snapshot.sources[0].impressions, None);
    }

    #[test]
    fn source_catalog_joins_builtin_impressions_by_logical_skill_name() {
        let directory = tempfile::tempdir().unwrap();
        let builtin = directory
            .path()
            .join("rust/loopflow/src/engine/builtins/build/skill/assess.md");
        fs::create_dir_all(builtin.parent().unwrap()).unwrap();
        fs::write(&builtin, "Assess instructions\n").unwrap();
        let current_hash = hash_current_source(
            ContextAssetKind::SkillInstructions,
            builtin.to_str().unwrap(),
        )
        .unwrap()
        .effective;

        let repo = directory.path().canonicalize().unwrap();
        let mut selection = query();
        selection.repo_paths = vec![repo.to_string_lossy().to_string()];
        let mut first_agent_session = launch("launch-a", "run-a", "completed", "complete", 100);
        first_agent_session.repo = repo.to_string_lossy().to_string();
        first_agent_session.worktree = first_agent_session.repo.clone();
        let mut second_agent_session = launch("launch-b", "run-b", "completed", "complete", 101);
        second_agent_session.repo = repo.to_string_lossy().to_string();
        second_agent_session.worktree = second_agent_session.repo.clone();
        let snapshot = aggregate(
            selection,
            vec![first_agent_session, second_agent_session],
            vec![
                turn("turn-a", "launch-a", 1, "assembled", 20, None),
                turn("turn-b", "launch-b", 1, "assembled", 20, None),
            ],
            vec![
                asset(
                    "turn-a",
                    0,
                    ContextAssetKind::SkillInstructions,
                    "assess",
                    None,
                    &current_hash,
                    20,
                ),
                asset(
                    "turn-b",
                    0,
                    ContextAssetKind::SkillInstructions,
                    "assess",
                    builtin.to_str(),
                    &current_hash,
                    20,
                ),
            ],
        );

        assert_eq!(snapshot.sources.len(), 1);
        let source = &snapshot.sources[0];
        assert_eq!(source.label, "assess");
        assert_eq!(source.impressions, Some(2));
        assert_eq!(source.observed_revisions, 1);
        assert_eq!(snapshot.coverage.source_observable_agent_sessions, 2);
        let current = snapshot
            .evidence
            .iter()
            .find(|item| Some(&item.node_id) == source.current_revision_node_id.as_ref())
            .unwrap();
        assert_eq!(
            current.source_path.as_deref(),
            builtin.canonicalize().unwrap().to_str()
        );
        assert_eq!(current.measurements.exposed_launches, 2);
        assert_eq!(
            current.current_content_sha256.as_deref(),
            Some(current.content_sha256.as_str())
        );
    }

    #[test]
    fn canonical_sources_collapse_task_worktrees_into_the_main_repo() {
        let mut launch = launch("launch-a", "run-a", "completed", "complete", 100);
        launch.repo = "/src/loopflow".to_string();
        launch.worktree = "/src/loopflow.intelligence.context".to_string();
        let row = asset(
            "turn-a",
            0,
            ContextAssetKind::OperatingInstructions,
            "LOOPFLOW.md",
            Some(
                "/src/loopflow.intelligence.context/rust/loopflow/src/engine/builtins/LOOPFLOW.md",
            ),
            "hash",
            20,
        );

        let identity = canonical_identity(&launch, &row);

        assert_eq!(
            identity.path.as_deref(),
            Some("/src/loopflow/rust/loopflow/src/engine/builtins/LOOPFLOW.md")
        );
    }

    #[test]
    fn current_source_hashes_keep_file_and_effective_identity_separate() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("LOOPFLOW.md");
        fs::write(&path, "Guide\n").unwrap();

        let hashes = hash_current_source(
            ContextAssetKind::OperatingInstructions,
            path.to_str().unwrap(),
        )
        .unwrap();

        assert_eq!(
            hashes.source,
            "3274fcad886cde4e2ca86b11d30fd7c44858eadf1c437a9583f31e7815db1af6"
        );
        assert_eq!(
            hashes.effective,
            "b779142188232028405d7f6245309de84876fde2d27c0df5c0c807bab73194ae"
        );
    }

    #[test]
    fn swift_fixture_round_trips_the_context_lab_contract() {
        let fixture = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/context_lab_snapshot.json"
        ));
        let snapshot: ContextLabSnapshot = serde_json::from_str(fixture).unwrap();

        assert_eq!(snapshot.totals.runs, 1);
        assert!(snapshot.query.steered_only);
        assert!(snapshot.query.current_revision_only);
        assert_eq!(snapshot.sessions[0].turns[1].supplied_context_tokens, None);
        assert_eq!(snapshot.sources[0].impressions, Some(1));
        assert_eq!(snapshot.sources[1].impressions, None);
        assert_eq!(
            snapshot.sources[0].current_revision_node_id.as_deref(),
            Some("context-revision")
        );
        assert_eq!(
            snapshot.evidence[0].representatives[0].address.turn_id,
            "turn-1"
        );
        assert_eq!(
            snapshot.evidence[0].measurements.provider_models[0].provider,
            "codex"
        );
        assert_eq!(
            serde_json::to_value(&snapshot).unwrap(),
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
    }

    fn query() -> SessionSetQuery {
        SessionSetQuery {
            repo_paths: Vec::new(),
            started_after: 0,
            started_before: 1_000,
            waves: Vec::new(),
            projects: Vec::new(),
            tasks: Vec::new(),
            flows: Vec::new(),
            skills: Vec::new(),
            providers: Vec::new(),
            models: Vec::new(),
            surfaces: Vec::new(),
            outcomes: Vec::new(),
            capture_states: Vec::new(),
            steered_only: false,
            current_revision_only: false,
        }
    }

    #[test]
    fn project_and_task_filters_require_durable_launch_attribution() {
        let attributed = launch("launch-1", "run-1", "completed", "complete", 100);
        let mut selection = query();
        selection.projects = vec!["context".to_string()];
        selection.tasks = vec!["W2-71".to_string()];
        assert!(launch_matches(&attributed, &selection));

        let mut unattributed = attributed;
        unattributed.task = None;
        assert!(!launch_matches(&unattributed, &selection));
    }

    fn launch(
        id: &str,
        run_id: &str,
        outcome: &str,
        capture_status: &str,
        started_at: i64,
    ) -> AgentLaunchRow {
        AgentLaunchRow {
            id: id.to_string(),
            run_id: run_id.to_string(),
            process_id: format!("process-{id}"),
            started_at,
            ended_at: Some(started_at + 10),
            repo: "/tmp/context-lab".to_string(),
            worktree: "/tmp/context-lab".to_string(),
            wave: Some("intelligence".to_string()),
            flow: Some("implement".to_string()),
            skill: Some("implement".to_string()),
            project: Some("context".to_string()),
            task: Some("W2-71".to_string()),
            provider: "codex".to_string(),
            model: Some("gpt-5".to_string()),
            surface: "headless".to_string(),
            capture_status: capture_status.to_string(),
            incomplete_reason: None,
            outcome: outcome.to_string(),
            artifact_dir: "missing".to_string(),
            conversation_path: "missing/conversation.jsonl".to_string(),
            provider_events_path: None,
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 0,
            conversation_bytes: 0,
            control: None,
        }
    }

    fn turn(
        id: &str,
        launch_id: &str,
        ordinal: i64,
        coverage: &str,
        tokens: i64,
        cost_usd: Option<f64>,
    ) -> AgentTurnRow {
        AgentTurnRow {
            id: id.to_string(),
            launch_id: launch_id.to_string(),
            ordinal,
            provider_turn_id: None,
            started_at: 100 + ordinal,
            ended_at: Some(110 + ordinal),
            status: "completed".to_string(),
            input_op: if ordinal == 1 { "initial" } else { "message" }.to_string(),
            context_coverage: coverage.to_string(),
            tokenizer: "cl100k_base".to_string(),
            system_prompt_path: None,
            task_prompt_path: "missing/prompt.txt".to_string(),
            system_tokens: 0,
            task_tokens: tokens,
            supplied_context_tokens: tokens,
            provider_input_tokens: None,
            provider_total_input_tokens: None,
            peak_input_tokens: None,
            context_window_tokens: None,
            provider_output_tokens: None,
            reasoning_tokens: None,
            cache_read_tokens: None,
            cache_write_tokens: None,
            cost_usd,
            context_gather_ms: 1,
            context_render_ms: 1,
            context_persist_ms: 1,
            first_event_seq: None,
            last_event_seq: None,
            root_output: None,
            basis: None,
        }
    }

    fn asset(
        turn_id: &str,
        position: u32,
        kind: ContextAssetKind,
        label: &str,
        source_path: Option<&str>,
        content_sha256: &str,
        tokens: u64,
    ) -> ContextAssetRow {
        ContextAssetRow {
            turn_id: turn_id.to_string(),
            asset: ContextAsset {
                position,
                channel: ContextChannel::Task,
                kind,
                scope: ContextScope::Repo,
                label: label.to_string(),
                source_path: source_path.map(ToString::to_string),
                included_by: "docs".to_string(),
                content_sha256: content_sha256.to_string(),
                byte_start: 0,
                byte_end: tokens,
                bytes: tokens,
                isolated_tokens: tokens,
                attributed_tokens: tokens,
            },
        }
    }
}
