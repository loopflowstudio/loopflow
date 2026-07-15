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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextLabSnapshot {
    pub query: SessionSetQuery,
    pub coverage: ContextCoverageDto,
    pub totals: SessionSetTotals,
    pub aggregate_root: ContextFlameNode,
    pub sessions: Vec<SessionLane>,
    pub evidence: Vec<SourceEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextCoverageDto {
    pub complete_launches: u64,
    pub partial_launches: u64,
    pub prompt_only_launches: u64,
    pub capturing_launches: u64,
    pub assembled_turns: u64,
    pub provider_total_only_turns: u64,
    pub unknown_turns: u64,
    pub prompt_artifacts_available: u64,
    pub conversations_available: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSetTotals {
    pub sessions: u64,
    pub launches: u64,
    pub turns: u64,
    pub context_tokens: Option<u64>,
    pub median_context_tokens: Option<u64>,
    pub p95_context_tokens: Option<u64>,
    pub instruction_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub cost_turns: u64,
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
    pub session_count: u64,
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
    pub completed_launches: u64,
    pub failed_launches: u64,
    pub steering_turns: Option<u64>,
    pub complete_capture_launches: u64,
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
    mut launches: Vec<AgentLaunchRow>,
    turns: Vec<AgentTurnRow>,
    assets: Vec<ContextAssetRow>,
) -> ContextLabSnapshot {
    launches.sort_by_key(|launch| (launch.started_at, launch.id.clone()));
    let assembled_turns = turns
        .iter()
        .filter(|turn| turn.context_coverage == "assembled")
        .map(|turn| turn.id.as_str())
        .collect::<HashSet<_>>();
    let assets = assets
        .into_iter()
        .filter(|row| assembled_turns.contains(row.turn_id.as_str()))
        .collect::<Vec<_>>();
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

    let sessions = build_session_lanes(&launches, &turns, &assets_by_turn);
    let coverage = build_coverage(&launches, &turns);
    let totals = build_totals(&launches, &turns, &assets);
    let aggregate_root = build_flame(&launches, &turns, &revisions);
    let evidence = build_evidence(revisions);
    ContextLabSnapshot {
        query,
        coverage,
        totals,
        aggregate_root,
        sessions,
        evidence,
    }
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

fn build_coverage(launches: &[AgentLaunchRow], turns: &[AgentTurnRow]) -> ContextCoverageDto {
    ContextCoverageDto {
        complete_launches: count_launches(launches, "complete"),
        partial_launches: count_launches(launches, "partial"),
        prompt_only_launches: count_launches(launches, "prompt_only"),
        capturing_launches: count_launches(launches, "capturing"),
        assembled_turns: count_turn_coverage(turns, "assembled"),
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
    }
}

fn build_totals(
    launches: &[AgentLaunchRow],
    turns: &[AgentTurnRow],
    assets: &[ContextAssetRow],
) -> SessionSetTotals {
    let mut context_values = turns
        .iter()
        .filter(|turn| turn.context_coverage == "assembled")
        .map(|turn| nonnegative(turn.supplied_context_tokens))
        .collect::<Vec<_>>();
    context_values.sort_unstable();
    let cost_values = turns
        .iter()
        .filter_map(|turn| turn.cost_usd)
        .collect::<Vec<_>>();
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
    let context_tokens = (!context_values.is_empty()).then(|| context_values.iter().sum());
    let instruction_tokens = context_tokens.and_then(|context_tokens| {
        (attributed_tokens > 0 || context_tokens == 0).then_some(instruction_tokens)
    });
    SessionSetTotals {
        sessions: launches
            .iter()
            .map(|launch| launch.run_id.as_str())
            .collect::<HashSet<_>>()
            .len() as u64,
        launches: launches.len() as u64,
        turns: turns.len() as u64,
        context_tokens,
        median_context_tokens: percentile(&context_values, 50),
        p95_context_tokens: percentile(&context_values, 95),
        instruction_tokens,
        cost_usd: (!cost_values.is_empty()).then(|| cost_values.iter().sum()),
        cost_turns: cost_values.len() as u64,
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
            SessionLane {
                id: launch.id.clone(),
                run_id: launch.run_id.clone(),
                started_at: launch.started_at,
                outcome: launch.outcome.clone(),
                steering_turns: Some(steering_turns),
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
                        supplied_context_tokens: (turn.context_coverage == "assembled")
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
        label: row.asset.label.clone(),
        attributed_tokens: row.asset.attributed_tokens,
    }
}

#[derive(Debug, Clone, Default)]
struct FlameAccumulator {
    attributed_tokens: u64,
    sessions: BTreeSet<String>,
    turns: BTreeSet<String>,
}

impl FlameAccumulator {
    fn add(&mut self, run_id: &str, turn_id: &str, attributed_tokens: u64) {
        self.attributed_tokens += attributed_tokens;
        self.sessions.insert(run_id.to_string());
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
        revision
            .flame
            .add(&launch.run_id, &turn.id, row.asset.attributed_tokens);
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
                id: stable_id(&format!("source\0{}\0{canonical_key}", kind.as_str())),
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
    root_accumulator.sessions = launches
        .iter()
        .map(|launch| launch.run_id.clone())
        .collect();
    root_accumulator.turns = turns.iter().map(|turn| turn.id.clone()).collect();
    node_from_accumulator(
        FlameNodeDescriptor {
            id: "session-set".to_string(),
            level: ContextFlameLevel::SessionSet,
            kind: None,
            label: "Session set".to_string(),
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
        session_count: accumulator.sessions.len() as u64,
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
        accumulator.sessions.extend(child.sessions.iter().cloned());
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
                    exposed_sessions: revision.flame.sessions.len() as u64,
                    exposed_launches: launch_candidates.len() as u64,
                    exposed_turns: candidates.len() as u64,
                    attributed_tokens: revision.flame.attributed_tokens,
                    median_tokens_per_exposed_turn: percentile(&per_turn, 50),
                    p95_tokens_per_exposed_turn: percentile(&per_turn, 95),
                    first_seen: candidates
                        .iter()
                        .map(|candidate| candidate.started_at)
                        .min(),
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

fn select_representatives(candidates: &[EvidenceCandidate]) -> Vec<RepresentativeTrace> {
    let mut selected = Vec::new();
    if let Some(candidate) = candidates
        .iter()
        .filter(|candidate| {
            candidate.outcome == "completed"
                && candidate.capture == "complete"
                && candidate.steering_turns.unwrap_or(0) == 0
        })
        .min_by_key(|candidate| candidate.supplied_context_tokens.unwrap_or(u64::MAX))
    {
        selected.push(representative(EvidenceRole::SmoothComplete, candidate));
    }
    if let Some(candidate) = candidates
        .iter()
        .filter(|candidate| candidate.outcome == "completed")
        .max_by_key(|candidate| candidate.supplied_context_tokens.unwrap_or(0))
    {
        selected.push(representative(EvidenceRole::HighContextComplete, candidate));
    }
    if let Some(candidate) = candidates
        .iter()
        .filter(|candidate| {
            candidate.outcome == "failed"
                || candidate.outcome == "interrupted"
                || candidate.steering_turns.unwrap_or(0) > 0
        })
        .max_by_key(|candidate| candidate.selected_source_tokens)
    {
        selected.push(representative(EvidenceRole::FailedOrSteered, candidate));
    }
    if let Some(candidate) = candidates
        .iter()
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
        label: row.asset.label.clone(),
        path: None,
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

fn kind_label(kind: &str) -> String {
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

fn print_human(snapshot: &ContextLabSnapshot) {
    if snapshot.totals.launches == 0 {
        println!("No captured agent context in the selected session set.");
        return;
    }
    let totals = &snapshot.totals;
    println!(
        "SESSION SET  {} sessions  {} launches  {} turns",
        totals.sessions, totals.launches, totals.turns
    );
    println!(
        "CONTEXT      {} tokens / {} assembled turns  median {}  p95 {}",
        display_optional(totals.context_tokens),
        snapshot.coverage.assembled_turns,
        display_optional(totals.median_context_tokens),
        display_optional(totals.p95_context_tokens),
    );
    println!(
        "CAPTURE      {} complete  {} partial  {} prompt-only  prompts {}/{}  conversations {}/{}",
        snapshot.coverage.complete_launches,
        snapshot.coverage.partial_launches,
        snapshot.coverage.prompt_only_launches,
        snapshot.coverage.prompt_artifacts_available,
        totals.turns,
        snapshot.coverage.conversations_available,
        totals.launches,
    );
    println!("\nCONTEXT FLAME");
    for node in &snapshot.aggregate_root.children {
        println!(
            "  {:<24} {:>10} tokens  {:>5} sessions  {:>5} turns",
            node.label, node.attributed_tokens, node.session_count, node.turn_count
        );
    }
}

fn display_optional(value: Option<u64>) -> String {
    value.map_or_else(|| "missing".to_string(), |value| value.to_string())
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

        assert_eq!(snapshot.totals.context_tokens, Some(100));
        assert_eq!(snapshot.coverage.assembled_turns, 1);
        assert_eq!(snapshot.totals.cost_usd, Some(0.25));
        assert_eq!(snapshot.totals.cost_turns, 1);
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
        assert_eq!(kind.children[0].session_count, 2);
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
    fn percentiles_count_each_captured_turn_once() {
        assert_eq!(percentile(&[10, 20, 30, 40], 50), Some(20));
        assert_eq!(percentile(&[10, 20, 30, 40], 95), Some(40));
        assert_eq!(percentile(&[], 95), None);
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

        assert_eq!(snapshot.totals.sessions, 1);
        assert_eq!(snapshot.sessions[0].turns[1].supplied_context_tokens, None);
        assert_eq!(
            snapshot.evidence[0].representatives[0].address.turn_id,
            "turn-1"
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
