//! `lf performance` — one bounded performance and spend scorecard.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::engine::worktrees::main_repo_root;

const SCORECARD_SCHEMA: u32 = 1;
const POLICY_PATH: &str = "performance/budgets.json";

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct BudgetPolicy {
    schema_version: u32,
    window_days: u32,
    minimum_p95_samples: usize,
    metrics: BTreeMap<String, MetricBudget>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct MetricBudget {
    unit: String,
    p50: Option<f64>,
    p95: Option<f64>,
    maximum: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Verdict {
    Pass,
    Fail,
    Collecting,
    Unknown,
}

impl Verdict {
    fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Collecting => "COLLECTING",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PerformanceRow {
    id: String,
    label: String,
    provider: Option<String>,
    eligible: usize,
    measured: usize,
    p50: Option<f64>,
    p95: Option<f64>,
    budget: MetricBudget,
    verdict: Verdict,
    reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct PerformanceReport {
    schema_version: u32,
    repo: String,
    window_started_at: String,
    window_ended_at: String,
    window_days: u32,
    minimum_p95_samples: usize,
    rows: Vec<PerformanceRow>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct GateRun {
    schema: u32,
    kind: String,
    finished_at: Option<String>,
    status: String,
    phases: Vec<GatePhase>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct GatePhase {
    phase: String,
    elapsed_s: f64,
    status: String,
}

#[derive(Debug, Clone, PartialEq)]
struct UsageSample {
    provider: String,
    total_input_tokens: Option<f64>,
    output_tokens: Option<f64>,
    cost_usd: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
struct ScorecardInput {
    generated_at: OffsetDateTime,
    repo: String,
    usage: Vec<UsageSample>,
    gates: Vec<GateRun>,
}

pub fn run(json: bool, repo: &Path) -> Result<()> {
    let policy = load_policy(repo)?;
    let generated_at = OffsetDateTime::now_utc();
    let main_repo = main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let since = generated_at.unix_timestamp() - i64::from(policy.window_days) * 86_400;
    let usage = load_usage(&main_repo, since)?;
    let gates = load_gates(&main_repo)?;
    let report = build_report(
        &policy,
        ScorecardInput {
            generated_at,
            repo: main_repo
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("loopflow")
                .to_string(),
            usage,
            gates,
        },
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_report(&report);
    }
    Ok(())
}

fn load_policy(repo: &Path) -> Result<BudgetPolicy> {
    let path = repo.join(POLICY_PATH);
    let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let policy: BudgetPolicy =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    if policy.schema_version != 1 {
        return Err(anyhow!(
            "unsupported performance budget schema {}",
            policy.schema_version
        ));
    }
    Ok(policy)
}

fn load_usage(repo: &Path, since: i64) -> Result<Vec<UsageSample>> {
    let database = crate::store::observability_database_path()?;
    let store = crate::store::sqlite::SqliteStore::open_run_ledger_read_only(&database)?;
    let mut repo_membership = HashMap::new();
    let invocations = store
        .agent_invocations_with_turns_ended_since(since)?
        .into_iter()
        .filter(|invocation| {
            *repo_membership
                .entry(invocation.repo.clone())
                .or_insert_with(|| invocation_belongs_to_repo(&invocation.repo, repo))
        })
        .collect::<Vec<_>>();
    let providers = invocations
        .iter()
        .map(|invocation| (invocation.id.clone(), invocation.provider.clone()))
        .collect::<HashMap<_, _>>();
    let invocation_ids = providers.keys().cloned().collect::<Vec<_>>();
    let turns = store.agent_turns_for_invocations(&invocation_ids)?;
    Ok(turns
        .into_iter()
        .filter(|turn| turn_ended_in_window(turn.ended_at, since))
        .filter_map(|turn| {
            let provider = providers.get(&turn.invocation_id)?.clone();
            Some(UsageSample {
                provider,
                total_input_tokens: turn.provider_total_input_tokens.map(|value| value as f64),
                output_tokens: turn.provider_output_tokens.map(|value| value as f64),
                cost_usd: turn.cost_usd,
            })
        })
        .collect())
}

fn turn_ended_in_window(ended_at: Option<i64>, since: i64) -> bool {
    ended_at.is_some_and(|ended_at| ended_at >= since)
}

fn invocation_belongs_to_repo(invocation_repo: &str, repo: &Path) -> bool {
    let invocation_repo = Path::new(invocation_repo);
    if invocation_repo == repo {
        return true;
    }
    let sibling_prefix = repo
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("{name}."));
    let is_loopflow_sibling = invocation_repo.parent() == repo.parent()
        && invocation_repo
            .file_name()
            .and_then(|name| name.to_str())
            .zip(sibling_prefix.as_deref())
            .is_some_and(|(name, prefix)| name.starts_with(prefix));
    is_loopflow_sibling || main_repo_root(invocation_repo).is_ok_and(|main_repo| main_repo == repo)
}

fn load_gates(repo: &Path) -> Result<Vec<GateRun>> {
    let root = repo.join(".git/loopflow/pre-land/runs");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for kind in ["changed", "full"] {
        let directory = root.join(kind);
        if !directory.exists() {
            continue;
        }
        for entry in
            fs::read_dir(&directory).with_context(|| format!("read {}", directory.display()))?
        {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    paths.into_iter().map(|path| load_gate(&path)).collect()
}

fn load_gate(path: &Path) -> Result<GateRun> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let gate: GateRun =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    if !matches!(gate.schema, 1 | 2) {
        return Err(anyhow!(
            "unsupported pre-land evidence schema {} in {}",
            gate.schema,
            path.display()
        ));
    }
    Ok(gate)
}

fn build_report(policy: &BudgetPolicy, input: ScorecardInput) -> Result<PerformanceReport> {
    let window_started = input.generated_at - time::Duration::days(i64::from(policy.window_days));
    let mut rows = vec![unknown_row(
        policy,
        "task_first_progress_seconds",
        "Task launch → first progress",
        "runs do not yet persist the first material provider event",
    )?];

    rows.push(gate_row(
        policy,
        &input.gates,
        "changed",
        "preland_changed_seconds",
        "Pre-land · changed",
        window_started,
    )?);
    rows.push(gate_row(
        policy,
        &input.gates,
        "full",
        "preland_full_seconds",
        "Pre-land · full",
        window_started,
    )?);
    rows.extend(phase_rows(policy, &input.gates, window_started));

    rows.extend([
        unknown_row(
            policy,
            "land_to_merge_seconds",
            "Land request → merge",
            "task_prs records the request and merge commit, but not GitHub mergedAt",
        )?,
        unknown_row(
            policy,
            "avoidable_repairs",
            "Avoidable repair",
            "best-effort worktree JSONL has no durable landing denominator",
        )?,
        unknown_row(
            policy,
            "credential_expiry_blocks",
            "Credential-expiry block",
            "provider account state has no incident history",
        )?,
        unknown_row(
            policy,
            "manual_git_repairs",
            "Manual git repair",
            "raw git adoption is not yet a durable scored incident",
        )?,
        unknown_row(
            policy,
            "build_disk_bytes",
            "Build + disk use",
            "local resource envelopes are owned by LOO-9",
        )?,
        unknown_row(
            policy,
            "preland_cpu_seconds",
            "Pre-land CPU",
            "gate evidence records wall time but not process CPU; collection is owned by LOO-9",
        )?,
    ]);

    rows.extend(usage_rows(policy, &input.usage, None)?);
    let providers = input
        .usage
        .iter()
        .map(|sample| sample.provider.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for provider in providers {
        rows.extend(usage_rows(policy, &input.usage, Some(provider))?);
    }

    Ok(PerformanceReport {
        schema_version: SCORECARD_SCHEMA,
        repo: input.repo,
        window_started_at: format_time(window_started)?,
        window_ended_at: format_time(input.generated_at)?,
        window_days: policy.window_days,
        minimum_p95_samples: policy.minimum_p95_samples,
        rows,
    })
}

fn gate_row(
    policy: &BudgetPolicy,
    gates: &[GateRun],
    kind: &str,
    metric: &str,
    label: &str,
    since: OffsetDateTime,
) -> Result<PerformanceRow> {
    let eligible = gates
        .iter()
        .filter(|gate| gate.kind == kind && gate_in_window(gate, since))
        .collect::<Vec<_>>();
    let values = eligible
        .iter()
        .filter(|gate| gate.status == "passed")
        .filter(|gate| gate.phases.iter().all(|phase| phase.status == "passed"))
        .map(|gate| gate.phases.iter().map(|phase| phase.elapsed_s).sum())
        .collect::<Vec<_>>();
    Ok(measured_row(
        metric,
        label,
        None,
        values,
        eligible.len(),
        metric_budget(policy, metric)?,
        policy.minimum_p95_samples,
    ))
}

fn phase_rows(
    policy: &BudgetPolicy,
    gates: &[GateRun],
    since: OffsetDateTime,
) -> Vec<PerformanceRow> {
    policy
        .metrics
        .iter()
        .filter_map(|(metric, budget)| {
            let phase_name = metric.strip_prefix("preland_phase.")?;
            let phases = gates
                .iter()
                .filter(|gate| gate_in_window(gate, since))
                .flat_map(|gate| gate.phases.iter())
                .filter(|phase| phase.phase == *phase_name)
                .collect::<Vec<_>>();
            let values = phases
                .iter()
                .filter(|phase| phase.status != "not_run")
                .map(|phase| phase.elapsed_s)
                .collect::<Vec<_>>();
            Some(measured_row(
                metric,
                &format!("Pre-land phase · {phase_name}"),
                None,
                values,
                phases.len(),
                budget.clone(),
                policy.minimum_p95_samples,
            ))
        })
        .collect()
}

fn gate_in_window(gate: &GateRun, since: OffsetDateTime) -> bool {
    gate.finished_at
        .as_deref()
        .and_then(|value| OffsetDateTime::parse(value, &Rfc3339).ok())
        .is_some_and(|finished| finished >= since)
}

fn usage_rows(
    policy: &BudgetPolicy,
    usage: &[UsageSample],
    provider: Option<&str>,
) -> Result<Vec<PerformanceRow>> {
    let samples = usage
        .iter()
        .filter(|sample| provider.is_none_or(|provider| sample.provider == provider))
        .collect::<Vec<_>>();
    let suffix = provider
        .map(|provider| format!(".{provider}"))
        .unwrap_or_default();
    let fields = [
        (
            "agent_total_input_tokens",
            "Agent total input / Turn",
            samples
                .iter()
                .filter_map(|sample| sample.total_input_tokens)
                .collect::<Vec<_>>(),
        ),
        (
            "agent_output_tokens",
            "Agent output / Turn",
            samples
                .iter()
                .filter_map(|sample| sample.output_tokens)
                .collect::<Vec<_>>(),
        ),
        (
            "agent_cost_usd",
            "Reported agent cost / Turn",
            samples
                .iter()
                .filter_map(|sample| sample.cost_usd)
                .collect::<Vec<_>>(),
        ),
    ];
    fields
        .into_iter()
        .map(|(metric, label, values)| {
            Ok(measured_row(
                &format!("{metric}{suffix}"),
                label,
                provider.map(str::to_string),
                values,
                samples.len(),
                metric_budget(policy, metric)?,
                policy.minimum_p95_samples,
            ))
        })
        .collect()
}

fn unknown_row(
    policy: &BudgetPolicy,
    metric: &str,
    label: &str,
    reason: &str,
) -> Result<PerformanceRow> {
    let budget = metric_budget(policy, metric)?;
    Ok(PerformanceRow {
        id: metric.to_string(),
        label: label.to_string(),
        provider: None,
        eligible: 0,
        measured: 0,
        p50: None,
        p95: None,
        budget,
        verdict: Verdict::Unknown,
        reason: Some(reason.to_string()),
    })
}

fn measured_row(
    id: &str,
    label: &str,
    provider: Option<String>,
    mut values: Vec<f64>,
    eligible: usize,
    budget: MetricBudget,
    minimum_p95_samples: usize,
) -> PerformanceRow {
    values.sort_by(f64::total_cmp);
    let p50 = percentile(&values, 0.50);
    let p95 = percentile(&values, 0.95);
    let measured = values.len();
    let breach = budget
        .p50
        .zip(p50)
        .is_some_and(|(limit, value)| value > limit)
        || budget
            .p95
            .zip(p95)
            .is_some_and(|(limit, value)| value > limit)
        || budget
            .maximum
            .is_some_and(|limit| values.iter().any(|value| *value > limit));
    let (verdict, reason) = if breach {
        (
            Verdict::Fail,
            Some("observed value exceeds budget".to_string()),
        )
    } else if eligible == 0 {
        (
            Verdict::Unknown,
            Some("no eligible evidence in window".to_string()),
        )
    } else if measured < eligible {
        (
            Verdict::Unknown,
            Some(format!(
                "{} of {eligible} eligible samples are missing",
                eligible - measured
            )),
        )
    } else if measured < minimum_p95_samples {
        (
            Verdict::Collecting,
            Some(format!(
                "{measured} samples; p95 requires {minimum_p95_samples}"
            )),
        )
    } else {
        (Verdict::Pass, None)
    };
    PerformanceRow {
        id: id.to_string(),
        label: label.to_string(),
        provider,
        eligible,
        measured,
        p50,
        p95,
        budget,
        verdict,
        reason,
    }
}

fn percentile(values: &[f64], quantile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let rank = (quantile * values.len() as f64).ceil() as usize;
    values.get(rank.saturating_sub(1)).copied()
}

fn metric_budget(policy: &BudgetPolicy, metric: &str) -> Result<MetricBudget> {
    policy
        .metrics
        .get(metric)
        .cloned()
        .ok_or_else(|| anyhow!("performance budget '{metric}' is missing from {POLICY_PATH}"))
}

fn format_time(value: OffsetDateTime) -> Result<String> {
    value.format(&Rfc3339).map_err(anyhow::Error::from)
}

fn print_report(report: &PerformanceReport) {
    println!(
        "Performance · {} · {} days through {}\n",
        report.repo, report.window_days, report.window_ended_at
    );
    println!(
        "{:<38}  {:>10}  {:>17}  {:>17}  {:>10}",
        "MEASURE", "COVERAGE", "P50 / BUDGET", "P95 / BUDGET", "VERDICT"
    );
    for row in &report.rows {
        let label = row.provider.as_ref().map_or_else(
            || row.label.clone(),
            |provider| format!("{} · {provider}", row.label),
        );
        println!(
            "{:<38}  {:>10}  {:>17}  {:>17}  {:>10}",
            crate::lf::output::truncate(&label, 38),
            format!("{}/{}", row.measured, row.eligible),
            format_value_budget(row.p50, row.budget.p50, &row.budget.unit),
            format_value_budget(
                row.p95,
                row.budget.p95.or(row.budget.maximum),
                &row.budget.unit,
            ),
            row.verdict.label(),
        );
        if let Some(reason) = &row.reason {
            println!("  {reason}");
        }
    }
}

fn format_value_budget(value: Option<f64>, budget: Option<f64>, unit: &str) -> String {
    format!(
        "{} / {}",
        value.map_or_else(|| "—".to_string(), |value| format_value(value, unit)),
        budget.map_or_else(|| "—".to_string(), |value| format_value(value, unit)),
    )
}

fn format_value(value: f64, unit: &str) -> String {
    match unit {
        "seconds" => format!("{value:.1}s"),
        "tokens" => crate::lf::output::format_int(value.round() as u64),
        "usd" => format!("${value:.2}"),
        "bytes" => format!("{:.1}GiB", value / 1024.0_f64.powi(3)),
        _ => format!("{value:.0}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_report, invocation_belongs_to_repo, load_gate, turn_ended_in_window, BudgetPolicy,
        GatePhase, GateRun, PerformanceRow, ScorecardInput, UsageSample, Verdict,
    };
    use serde::Deserialize;
    use std::path::Path;
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    const POLICY: &str = include_str!("../../../../../performance/budgets.json");
    const GOLDEN: &str = include_str!("../../../../../tests/fixtures/performance_scorecard.json");

    #[derive(Debug, PartialEq, Deserialize)]
    struct GoldenRow {
        id: String,
        eligible: usize,
        measured: usize,
        p50: Option<f64>,
        p95: Option<f64>,
        verdict: Verdict,
        reason: Option<String>,
    }

    impl From<&PerformanceRow> for GoldenRow {
        fn from(row: &PerformanceRow) -> Self {
            Self {
                id: row.id.clone(),
                eligible: row.eligible,
                measured: row.measured,
                p50: row.p50,
                p95: row.p95,
                verdict: row.verdict,
                reason: row.reason.clone(),
            }
        }
    }

    #[test]
    fn historical_loopflow_worktrees_belong_to_the_main_repo() {
        let repo = Path::new("/src/loopflow");

        assert!(invocation_belongs_to_repo("/src/loopflow", repo));
        assert!(invocation_belongs_to_repo(
            "/src/loopflow.make-performance-visible",
            repo
        ));
        assert!(!invocation_belongs_to_repo("/src/cadenza.worker", repo));
    }

    #[test]
    fn usage_window_is_based_on_terminal_turn_time() {
        assert!(!turn_ended_in_window(None, 100));
        assert!(!turn_ended_in_window(Some(99), 100));
        assert!(turn_ended_in_window(Some(100), 100));
    }

    #[test]
    fn reads_the_gate_record_emitted_by_the_landing_path() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("gate.json");
        std::fs::write(
            &path,
            r#"{
              "schema": 2,
              "run_id": "gate-real-path",
              "kind": "changed",
              "branch": "jack/performance",
              "head": "abc123",
              "worktree": "/src/loopflow.performance",
              "tree_fingerprint": "tree",
              "plan_fingerprint": "plan",
              "started_at": "2026-07-21T18:00:00Z",
              "finished_at": "2026-07-21T18:02:00Z",
              "status": "passed",
              "phases": [{
                "suite": "rust",
                "phase": "rust",
                "budget_s": 1200,
                "elapsed_s": 120.0,
                "status": "passed",
                "over_budget": false
              }]
            }"#,
        )
        .unwrap();

        let gate = load_gate(&path).unwrap();

        assert_eq!(gate.kind, "changed");
        assert_eq!(gate.status, "passed");
        assert_eq!(gate.phases[0].elapsed_s, 120.0);
    }

    #[test]
    fn golden_scorecard_preserves_missing_zero_and_budget_verdicts() {
        let policy: BudgetPolicy = serde_json::from_str(POLICY).unwrap();
        let generated_at = OffsetDateTime::parse("2026-07-21T19:00:00Z", &Rfc3339).unwrap();
        let usage = vec![
            UsageSample {
                provider: "claude".to_string(),
                total_input_tokens: Some(0.0),
                output_tokens: Some(0.0),
                cost_usd: Some(0.0),
            },
            UsageSample {
                provider: "claude".to_string(),
                total_input_tokens: Some(6_000_000.0),
                output_tokens: Some(25_000.0),
                cost_usd: Some(9.0),
            },
            UsageSample {
                provider: "codex".to_string(),
                total_input_tokens: None,
                output_tokens: Some(0.0),
                cost_usd: None,
            },
        ];
        let gates = vec![GateRun {
            schema: 2,
            kind: "full".to_string(),
            finished_at: Some("2026-07-20T00:05:00Z".to_string()),
            status: "passed".to_string(),
            phases: vec![GatePhase {
                phase: "rust".to_string(),
                elapsed_s: 300.0,
                status: "passed".to_string(),
            }],
        }];

        let report = build_report(
            &policy,
            ScorecardInput {
                generated_at,
                repo: "loopflow".to_string(),
                usage,
                gates,
            },
        )
        .unwrap();
        let expected: Vec<GoldenRow> = serde_json::from_str(GOLDEN).unwrap();
        let actual = expected
            .iter()
            .map(|expected| {
                report
                    .rows
                    .iter()
                    .find(|row| row.id == expected.id)
                    .map(GoldenRow::from)
                    .expect("golden metric is present")
            })
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}
