//! Provider-billed token usage derived from one cumulative checkpoint ledger.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::store::sqlite::SqliteStore;
use crate::store::{StoreResult, TurnUsageSample};
use crate::trace::{AgentInvocationRow, AgentTurnRow};

pub const USAGE_SCHEMA_VERSION: u32 = 1;
pub const USAGE_WINDOWS: [i64; 4] = [
    5,
    300,
    3_600,
    crate::store::TURN_USAGE_LIVE_RETENTION_SECONDS,
];
pub const HISTORY_BUCKET_SECONDS: i64 = 300;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UsageScopeKind {
    Global,
    Repository,
    Wave,
    Project,
    Task,
    Exec,
    Invocation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageScope {
    pub id: String,
    pub parent_id: Option<String>,
    pub kind: UsageScopeKind,
    pub label: String,
    pub repo: Option<String>,
    pub wave: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub exec_id: Option<String>,
    pub invocation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageInterval {
    pub window_seconds: i64,
    pub input_tokens: Option<u64>,
    pub total_input_tokens: Option<u64>,
    pub output_tokens: u64,
    pub reasoning_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub peak_input_tokens: Option<u64>,
    pub context_window_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub output_tokens_per_second: f64,
    pub measured_turns: u64,
    pub unmeasured_turns: u64,
    pub output_complete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageReading {
    pub scope: UsageScope,
    pub intervals: Vec<UsageInterval>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageBucket {
    pub started_at: i64,
    pub ended_at: i64,
    pub input_tokens: Option<u64>,
    pub total_input_tokens: Option<u64>,
    pub output_tokens: u64,
    pub reasoning_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UsageSnapshot {
    pub schema_version: u32,
    pub observed_at: i64,
    pub windows: Vec<i64>,
    pub readings: Vec<UsageReading>,
    pub history_bucket_seconds: i64,
    pub global_history: Vec<UsageBucket>,
}

#[derive(Debug, Default)]
struct IntervalAccumulator {
    input_tokens: u64,
    input_measured: bool,
    total_input_tokens: u64,
    total_input_measured: bool,
    output_tokens: u64,
    reasoning_tokens: u64,
    reasoning_measured: bool,
    cache_read_tokens: u64,
    cache_read_measured: bool,
    cache_write_tokens: u64,
    cache_write_measured: bool,
    peak_input_tokens: Option<u64>,
    context_window_tokens: Option<u64>,
    cost_usd: f64,
    cost_measured: bool,
    measured_turns: HashSet<String>,
    overlapping_turns: HashSet<String>,
}

#[derive(Debug)]
struct ReadingAccumulator {
    scope: UsageScope,
    intervals: Vec<IntervalAccumulator>,
}

#[derive(Debug, Default)]
struct BucketAccumulator {
    input_tokens: u64,
    input_measured: bool,
    total_input_tokens: u64,
    total_input_measured: bool,
    output_tokens: u64,
    reasoning_tokens: u64,
    reasoning_measured: bool,
    cache_read_tokens: u64,
    cache_read_measured: bool,
    cache_write_tokens: u64,
    cache_write_measured: bool,
    cost_usd: f64,
    cost_measured: bool,
}

pub fn snapshot(store: &SqliteStore, now: i64) -> StoreResult<UsageSnapshot> {
    let oldest = now - USAGE_WINDOWS[USAGE_WINDOWS.len() - 1];
    let samples = store.turn_usage_samples_since(oldest)?;
    let invocations = store.agent_invocations_overlapping_since(oldest)?;
    let invocation_ids = invocations
        .iter()
        .map(|invocation| invocation.id.clone())
        .collect::<Vec<_>>();
    let turns = store.agent_turns_for_invocations(&invocation_ids)?;
    Ok(_build_snapshot(now, &samples, &invocations, &turns))
}

fn _build_snapshot(
    now: i64,
    samples: &[TurnUsageSample],
    invocations: &[AgentInvocationRow],
    turns: &[AgentTurnRow],
) -> UsageSnapshot {
    let history_start = now.div_euclid(HISTORY_BUCKET_SECONDS) * HISTORY_BUCKET_SECONDS
        - (USAGE_WINDOWS[USAGE_WINDOWS.len() - 1] - HISTORY_BUCKET_SECONDS);
    let invocation_by_id = invocations
        .iter()
        .map(|invocation| (invocation.id.as_str(), invocation))
        .collect::<HashMap<_, _>>();
    let mut readings = BTreeMap::<String, ReadingAccumulator>::new();
    _ensure_scope(&mut readings, _global_scope());

    _record_turn_lifecycle(&mut readings, turns, &invocation_by_id, now);
    let mut buckets = (0..USAGE_WINDOWS[USAGE_WINDOWS.len() - 1] / HISTORY_BUCKET_SECONDS)
        .map(|_| BucketAccumulator::default())
        .collect::<Vec<_>>();
    _record_deltas(
        &mut readings,
        &mut buckets,
        samples,
        turns,
        &invocation_by_id,
        history_start,
        now,
    );

    let readings = readings
        .into_values()
        .map(|reading| UsageReading {
            scope: reading.scope,
            intervals: reading
                .intervals
                .into_iter()
                .zip(USAGE_WINDOWS)
                .map(|(interval, window_seconds)| {
                    let unmeasured_turns = interval
                        .overlapping_turns
                        .difference(&interval.measured_turns)
                        .count() as u64;
                    UsageInterval {
                        window_seconds,
                        input_tokens: interval.input_measured.then_some(interval.input_tokens),
                        total_input_tokens: interval
                            .total_input_measured
                            .then_some(interval.total_input_tokens),
                        output_tokens: interval.output_tokens,
                        reasoning_tokens: interval
                            .reasoning_measured
                            .then_some(interval.reasoning_tokens),
                        cache_read_tokens: interval
                            .cache_read_measured
                            .then_some(interval.cache_read_tokens),
                        cache_write_tokens: interval
                            .cache_write_measured
                            .then_some(interval.cache_write_tokens),
                        peak_input_tokens: interval.peak_input_tokens,
                        context_window_tokens: interval.context_window_tokens,
                        cost_usd: interval.cost_measured.then_some(interval.cost_usd),
                        output_tokens_per_second: interval.output_tokens as f64
                            / window_seconds as f64,
                        measured_turns: interval.measured_turns.len() as u64,
                        unmeasured_turns,
                        output_complete: unmeasured_turns == 0,
                    }
                })
                .collect(),
        })
        .collect();

    let global_history = buckets
        .into_iter()
        .enumerate()
        .map(|(index, bucket)| {
            let started_at = history_start + index as i64 * HISTORY_BUCKET_SECONDS;
            UsageBucket {
                started_at,
                ended_at: started_at + HISTORY_BUCKET_SECONDS,
                input_tokens: bucket.input_measured.then_some(bucket.input_tokens),
                total_input_tokens: bucket
                    .total_input_measured
                    .then_some(bucket.total_input_tokens),
                output_tokens: bucket.output_tokens,
                reasoning_tokens: bucket.reasoning_measured.then_some(bucket.reasoning_tokens),
                cache_read_tokens: bucket
                    .cache_read_measured
                    .then_some(bucket.cache_read_tokens),
                cache_write_tokens: bucket
                    .cache_write_measured
                    .then_some(bucket.cache_write_tokens),
                cost_usd: bucket.cost_measured.then_some(bucket.cost_usd),
            }
        })
        .collect();

    UsageSnapshot {
        schema_version: USAGE_SCHEMA_VERSION,
        observed_at: now,
        windows: USAGE_WINDOWS.to_vec(),
        readings,
        history_bucket_seconds: HISTORY_BUCKET_SECONDS,
        global_history,
    }
}

pub fn empty_snapshot(now: i64) -> UsageSnapshot {
    UsageSnapshot {
        schema_version: USAGE_SCHEMA_VERSION,
        observed_at: now,
        windows: USAGE_WINDOWS.to_vec(),
        readings: vec![UsageReading {
            scope: _global_scope(),
            intervals: USAGE_WINDOWS
                .into_iter()
                .map(|window_seconds| UsageInterval {
                    window_seconds,
                    input_tokens: None,
                    total_input_tokens: None,
                    output_tokens: 0,
                    reasoning_tokens: None,
                    cache_read_tokens: None,
                    cache_write_tokens: None,
                    peak_input_tokens: None,
                    context_window_tokens: None,
                    cost_usd: None,
                    output_tokens_per_second: 0.0,
                    measured_turns: 0,
                    unmeasured_turns: 0,
                    output_complete: true,
                })
                .collect(),
        }],
        history_bucket_seconds: HISTORY_BUCKET_SECONDS,
        global_history: Vec::new(),
    }
}

fn _record_turn_lifecycle(
    readings: &mut BTreeMap<String, ReadingAccumulator>,
    turns: &[AgentTurnRow],
    invocation_by_id: &HashMap<&str, &AgentInvocationRow>,
    now: i64,
) {
    for turn in turns {
        let Some(invocation) = invocation_by_id.get(turn.invocation_id.as_str()) else {
            continue;
        };
        let relevant_windows = USAGE_WINDOWS
            .into_iter()
            .enumerate()
            .filter_map(|(index, window_seconds)| {
                let boundary = now - window_seconds;
                let started_in_window = (boundary..=now).contains(&turn.started_at);
                let ended_in_window = turn
                    .ended_at
                    .is_some_and(|ended| (boundary..=now).contains(&ended));
                (started_in_window || ended_in_window).then_some(index)
            })
            .collect::<Vec<_>>();
        if relevant_windows.is_empty() {
            continue;
        }
        let scopes = _scope_chain(invocation);
        for scope in &scopes {
            _ensure_scope(readings, scope.clone());
        }
        for index in relevant_windows {
            for scope in &scopes {
                readings
                    .get_mut(&scope.id)
                    .expect("scope was ensured")
                    .intervals[index]
                    .overlapping_turns
                    .insert(turn.id.clone());
            }
        }
    }
}

fn _record_deltas(
    readings: &mut BTreeMap<String, ReadingAccumulator>,
    buckets: &mut [BucketAccumulator],
    samples: &[TurnUsageSample],
    turns: &[AgentTurnRow],
    invocation_by_id: &HashMap<&str, &AgentInvocationRow>,
    history_start: i64,
    now: i64,
) {
    let oldest = now - USAGE_WINDOWS[USAGE_WINDOWS.len() - 1];
    let invocation_for_turn = turns
        .iter()
        .map(|turn| (turn.id.as_str(), turn.invocation_id.as_str()))
        .collect::<HashMap<_, _>>();
    let mut previous_usage = HashMap::<&str, crate::chat::types::TurnUsage>::new();

    for sample in samples {
        let previous = previous_usage.get(sample.turn_id.as_str());
        let input_delta = _token_delta(
            sample.usage.input_tokens,
            previous.and_then(|usage| usage.input_tokens),
        );
        let total_input_delta = _token_delta(
            sample.usage.total_input_tokens,
            previous.and_then(|usage| usage.total_input_tokens),
        );
        let output_delta = _token_delta(
            sample.usage.output_tokens,
            previous.and_then(|usage| usage.output_tokens),
        );
        let reasoning_delta = _token_delta(
            sample.usage.reasoning_tokens,
            previous.and_then(|usage| usage.reasoning_tokens),
        );
        let cache_read_delta = _token_delta(
            sample.usage.cache_read_tokens,
            previous.and_then(|usage| usage.cache_read_tokens),
        );
        let cache_write_delta = _token_delta(
            sample.usage.cache_write_tokens,
            previous.and_then(|usage| usage.cache_write_tokens),
        );
        let cost_delta = sample
            .usage
            .cost_usd
            .map(|cost| (cost - previous.and_then(|usage| usage.cost_usd).unwrap_or(0.0)).max(0.0));
        previous_usage.insert(sample.turn_id.as_str(), sample.usage.clone());
        if sample.observed_at < oldest || sample.observed_at > now {
            continue;
        }
        let Some(invocation_id) = invocation_for_turn.get(sample.turn_id.as_str()) else {
            continue;
        };
        let Some(invocation) = invocation_by_id.get(*invocation_id) else {
            continue;
        };
        let scopes = _scope_chain(invocation);
        for scope in &scopes {
            _ensure_scope(readings, scope.clone());
        }
        for (index, window_seconds) in USAGE_WINDOWS.into_iter().enumerate() {
            if sample.observed_at < now - window_seconds {
                continue;
            }
            for scope in &scopes {
                let Some(reading) = readings.get_mut(&scope.id) else {
                    continue;
                };
                let interval = &mut reading.intervals[index];
                interval.overlapping_turns.insert(sample.turn_id.clone());
                _add_optional(
                    &mut interval.input_tokens,
                    &mut interval.input_measured,
                    input_delta,
                );
                _add_optional(
                    &mut interval.total_input_tokens,
                    &mut interval.total_input_measured,
                    total_input_delta,
                );
                if let Some(delta) = output_delta {
                    interval.output_tokens = interval.output_tokens.saturating_add(delta);
                    interval.measured_turns.insert(sample.turn_id.clone());
                }
                if let Some(delta) = reasoning_delta {
                    interval.reasoning_tokens = interval.reasoning_tokens.saturating_add(delta);
                    interval.reasoning_measured = true;
                }
                _add_optional(
                    &mut interval.cache_read_tokens,
                    &mut interval.cache_read_measured,
                    cache_read_delta,
                );
                _add_optional(
                    &mut interval.cache_write_tokens,
                    &mut interval.cache_write_measured,
                    cache_write_delta,
                );
                if sample.usage.peak_input_tokens > interval.peak_input_tokens {
                    interval.peak_input_tokens = sample.usage.peak_input_tokens;
                    interval.context_window_tokens = sample.usage.context_window_tokens;
                }
                if let Some(delta) = cost_delta {
                    interval.cost_usd += delta;
                    interval.cost_measured = true;
                }
            }
        }
        if sample.observed_at < history_start {
            continue;
        }
        let bucket_index = ((sample.observed_at - history_start) / HISTORY_BUCKET_SECONDS) as usize;
        if let Some(bucket) = buckets.get_mut(bucket_index) {
            _add_optional(
                &mut bucket.input_tokens,
                &mut bucket.input_measured,
                input_delta,
            );
            _add_optional(
                &mut bucket.total_input_tokens,
                &mut bucket.total_input_measured,
                total_input_delta,
            );
            if let Some(delta) = output_delta {
                bucket.output_tokens = bucket.output_tokens.saturating_add(delta);
            }
            if let Some(delta) = reasoning_delta {
                bucket.reasoning_tokens = bucket.reasoning_tokens.saturating_add(delta);
                bucket.reasoning_measured = true;
            }
            _add_optional(
                &mut bucket.cache_read_tokens,
                &mut bucket.cache_read_measured,
                cache_read_delta,
            );
            _add_optional(
                &mut bucket.cache_write_tokens,
                &mut bucket.cache_write_measured,
                cache_write_delta,
            );
            if let Some(delta) = cost_delta {
                bucket.cost_usd += delta;
                bucket.cost_measured = true;
            }
        }
    }
}

fn _token_delta(current: Option<u64>, previous: Option<u64>) -> Option<u64> {
    current.map(|current| current.saturating_sub(previous.unwrap_or(0)))
}

fn _add_optional(total: &mut u64, measured: &mut bool, delta: Option<u64>) {
    if let Some(delta) = delta {
        *total = total.saturating_add(delta);
        *measured = true;
    }
}

fn _ensure_scope(readings: &mut BTreeMap<String, ReadingAccumulator>, scope: UsageScope) {
    readings
        .entry(scope.id.clone())
        .or_insert_with(|| ReadingAccumulator {
            scope,
            intervals: USAGE_WINDOWS
                .iter()
                .map(|_| IntervalAccumulator::default())
                .collect(),
        });
}

fn _scope_chain(invocation: &AgentInvocationRow) -> Vec<UsageScope> {
    let mut scopes = vec![_global_scope()];
    let repo_id = format!("repo:{}", invocation.repo);
    scopes.push(UsageScope {
        id: repo_id.clone(),
        parent_id: Some("global".to_string()),
        kind: UsageScopeKind::Repository,
        label: Path::new(&invocation.repo)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&invocation.repo)
            .to_string(),
        repo: Some(invocation.repo.clone()),
        wave: None,
        project: None,
        task: None,
        exec_id: None,
        invocation_id: None,
    });
    let mut parent_id = repo_id;
    if let Some(wave) = invocation.wave.as_ref() {
        let id = format!("wave:{}:{wave}", invocation.repo);
        scopes.push(UsageScope {
            id: id.clone(),
            parent_id: Some(parent_id),
            kind: UsageScopeKind::Wave,
            label: wave.clone(),
            repo: Some(invocation.repo.clone()),
            wave: Some(wave.clone()),
            project: None,
            task: None,
            exec_id: None,
            invocation_id: None,
        });
        parent_id = id;
        if let Some(project) = invocation.project.as_ref() {
            let id = format!("{parent_id}:project:{project}");
            scopes.push(UsageScope {
                id: id.clone(),
                parent_id: Some(parent_id),
                kind: UsageScopeKind::Project,
                label: project.clone(),
                repo: Some(invocation.repo.clone()),
                wave: invocation.wave.clone(),
                project: Some(project.clone()),
                task: None,
                exec_id: None,
                invocation_id: None,
            });
            parent_id = id;
            if let Some(task) = invocation.task.as_ref() {
                let id = format!("{parent_id}:task:{task}");
                scopes.push(UsageScope {
                    id: id.clone(),
                    parent_id: Some(parent_id),
                    kind: UsageScopeKind::Task,
                    label: task.clone(),
                    repo: Some(invocation.repo.clone()),
                    wave: invocation.wave.clone(),
                    project: invocation.project.clone(),
                    task: Some(task.clone()),
                    exec_id: None,
                    invocation_id: None,
                });
                parent_id = id;
            }
        }
    }
    let exec_id = format!("exec:{}", invocation.process_id);
    scopes.push(UsageScope {
        id: exec_id.clone(),
        parent_id: Some(parent_id),
        kind: UsageScopeKind::Exec,
        label: invocation.process_id.clone(),
        repo: Some(invocation.repo.clone()),
        wave: invocation.wave.clone(),
        project: invocation.project.clone(),
        task: invocation.task.clone(),
        exec_id: Some(invocation.process_id.clone()),
        invocation_id: None,
    });
    scopes.push(UsageScope {
        id: format!("invocation:{}", invocation.id),
        parent_id: Some(exec_id),
        kind: UsageScopeKind::Invocation,
        label: invocation
            .skill
            .clone()
            .unwrap_or_else(|| invocation.provider.clone()),
        repo: Some(invocation.repo.clone()),
        wave: invocation.wave.clone(),
        project: invocation.project.clone(),
        task: invocation.task.clone(),
        exec_id: Some(invocation.process_id.clone()),
        invocation_id: Some(invocation.id.clone()),
    });
    scopes
}

fn _global_scope() -> UsageScope {
    UsageScope {
        id: "global".to_string(),
        parent_id: None,
        kind: UsageScopeKind::Global,
        label: "All Loopflow".to_string(),
        repo: None,
        wave: None,
        project: None,
        task: None,
        exec_id: None,
        invocation_id: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::types::TurnUsage;

    fn invocation(
        id: &str,
        process: &str,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
    ) -> AgentInvocationRow {
        AgentInvocationRow {
            id: id.to_string(),
            run_id: format!("run-{id}"),
            answer_ask_id: None,
            process_id: process.to_string(),
            started_at: 500,
            ended_at: None,
            repo: "/src/loopflow".to_string(),
            worktree: "/src/loopflow".to_string(),
            wave: wave.map(str::to_string),
            flow: Some("implement".to_string()),
            skill: Some("implement".to_string()),
            project: project.map(str::to_string),
            task: task.map(str::to_string),
            provider: "codex".to_string(),
            model: Some("gpt-5".to_string()),
            surface: "headless".to_string(),
            capture_status: "capturing".to_string(),
            incomplete_reason: None,
            outcome: "running".to_string(),
            artifact_dir: format!("traces/{id}"),
            conversation_path: format!("traces/{id}/conversation.jsonl"),
            provider_events_path: Some(format!("traces/{id}/provider.jsonl")),
            provider_session_id: None,
            provider_session_path: None,
            conversation_event_count: 0,
            conversation_bytes: 0,
            supervision: None,
        }
    }

    fn turn(id: &str, invocation_id: &str) -> AgentTurnRow {
        AgentTurnRow {
            id: id.to_string(),
            invocation_id: invocation_id.to_string(),
            ordinal: 1,
            provider_turn_id: None,
            started_at: 500,
            ended_at: None,
            status: "running".to_string(),
            input_op: "initial".to_string(),
            context_coverage: "assembled".to_string(),
            tokenizer: "o200k_base".to_string(),
            system_prompt_path: None,
            task_prompt_path: "prompt.md".to_string(),
            system_tokens: 0,
            task_tokens: 0,
            supplied_context_tokens: 0,
            usage: None,
            context_gather_ms: 0,
            context_render_ms: 0,
            context_persist_ms: 0,
            first_event_seq: None,
            last_event_seq: None,
            root_output: None,
            basis: None,
        }
    }

    fn sample(turn_id: &str, observed_at: i64, input: u64, output: u64) -> TurnUsageSample {
        TurnUsageSample {
            turn_id: turn_id.to_string(),
            observed_at,
            final_receipt: false,
            usage: TurnUsage {
                input_tokens: Some(input),
                total_input_tokens: Some(input + 20),
                output_tokens: Some(output),
                reasoning_tokens: Some(output / 4),
                cache_read_tokens: Some(20),
                peak_input_tokens: Some(input + 20),
                context_window_tokens: Some(1_000),
                model: Some("gpt-5".to_string()),
                ..Default::default()
            },
        }
    }

    fn interval<'a>(
        snapshot: &'a UsageSnapshot,
        scope_id: &str,
        seconds: i64,
    ) -> &'a UsageInterval {
        snapshot
            .readings
            .iter()
            .find(|reading| reading.scope.id == scope_id)
            .and_then(|reading| {
                reading
                    .intervals
                    .iter()
                    .find(|interval| interval.window_seconds == seconds)
            })
            .expect("usage interval")
    }

    #[test]
    fn snapshot_counts_unattributed_output_only_in_global_ancestors() {
        let now = 1_000;
        let attributed = invocation(
            "invocation-attributed",
            "exec-attributed",
            Some("product"),
            Some("podium"),
            Some("LOO-1"),
        );
        let unattributed = invocation("invocation-global", "exec-global", None, None, None);
        let unmeasured = invocation("invocation-unmeasured", "exec-unmeasured", None, None, None);
        let stale = invocation("invocation-stale", "exec-stale", None, None, None);
        let mut recent_unmeasured_turn = turn("turn-unmeasured", &unmeasured.id);
        recent_unmeasured_turn.started_at = now - 1;
        let turns = vec![
            turn("turn-attributed", &attributed.id),
            turn("turn-global", &unattributed.id),
            recent_unmeasured_turn,
            turn("turn-stale", &stale.id),
        ];
        let samples = vec![
            sample("turn-attributed", now - 400, 20, 10),
            sample("turn-attributed", now - 2, 100, 40),
            sample("turn-global", now - 2, 5, 5),
        ];
        let snapshot = _build_snapshot(
            now,
            &samples,
            &[attributed, unattributed, unmeasured, stale],
            &turns,
        );

        let global = interval(&snapshot, "global", 5);
        assert_eq!(global.output_tokens, 35);
        assert_eq!(global.input_tokens, Some(85));
        assert_eq!(global.unmeasured_turns, 1);
        assert!(!global.output_complete);

        let wave = interval(&snapshot, "wave:/src/loopflow:product", 5);
        assert_eq!(wave.output_tokens, 30);
        assert_eq!(wave.input_tokens, Some(80));
        assert_eq!(wave.unmeasured_turns, 0);
        assert!(wave.output_complete);

        assert_eq!(snapshot.global_history.len(), 288);
        assert_eq!(snapshot.global_history.last().unwrap().started_at, 900);
        assert_eq!(snapshot.global_history.last().unwrap().ended_at, 1_200);
        assert_eq!(snapshot.global_history.last().unwrap().output_tokens, 35);
    }

    #[test]
    fn same_project_slug_in_different_waves_has_distinct_scope_ids() {
        let first = invocation(
            "first",
            "exec-first",
            Some("product"),
            Some("quality"),
            None,
        );
        let second = invocation(
            "second",
            "exec-second",
            Some("infrastructure"),
            Some("quality"),
            None,
        );
        let first_project = _scope_chain(&first)
            .into_iter()
            .find(|scope| scope.kind == UsageScopeKind::Project)
            .unwrap();
        let second_project = _scope_chain(&second)
            .into_iter()
            .find(|scope| scope.kind == UsageScopeKind::Project)
            .unwrap();

        assert_ne!(first_project.id, second_project.id);
        assert_eq!(
            first_project.parent_id.as_deref(),
            Some("wave:/src/loopflow:product")
        );
    }

    #[test]
    fn partial_work_attribution_never_invents_orphan_project_or_task_scopes() {
        let partial = invocation(
            "partial",
            "exec-partial",
            None,
            Some("quality"),
            Some("LOO-1"),
        );
        let scopes = _scope_chain(&partial);

        assert!(!scopes.iter().any(|scope| matches!(
            scope.kind,
            UsageScopeKind::Wave | UsageScopeKind::Project | UsageScopeKind::Task
        )));
        let exec = scopes
            .iter()
            .find(|scope| scope.kind == UsageScopeKind::Exec)
            .unwrap();
        assert_eq!(exec.parent_id.as_deref(), Some("repo:/src/loopflow"));
    }
}
