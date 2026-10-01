//! `lf usage --weekly` — what context costs, and what it does to turn time.
//!
//! Weeks are seven days anchored on the 2026-09-30 baseline, so every report
//! shows the same trend line. A step is one captured Session input; unknown
//! usage stays unmeasured rather than counting as zero.

use std::collections::BTreeMap;

use anyhow::Result;
use serde::Serialize;
use time::OffsetDateTime;

use crate::lf::commands::WorkFilter;
use crate::lf::output::{format_int, truncate, Colors};
use crate::ops::metrics::MetricProducerObservation;
use crate::session_record::SessionHistory;

/// 2026-09-23T00:00:00Z: the baseline week ends on 2026-09-30.
const FIRST_WEEK_START: i64 = 1_790_121_600;
const WEEK_SECONDS: i64 = 7 * 86_400;
const SILENCE_SECONDS: i64 = 600;
const BANDS: [(&str, i64); 4] = [
    ("<100k", 0),
    ("100-150k", 100_000),
    ("150-200k", 150_000),
    ("200k+", 200_000),
];
const SLOW_BAND: &str = "200k+";
const WAVE: &str = "intelligence";
const INSTRUMENT: &str = "context-cost";
const TASK_ROWS: usize = 10;

#[derive(Debug, Serialize)]
pub struct ContextCostReport {
    pub silence_threshold_seconds: i64,
    pub weeks: Vec<WeekCost>,
}

#[derive(Debug, Serialize)]
pub struct WeekCost {
    #[serde(with = "time::serde::rfc3339")]
    pub start: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub end: OffsetDateTime,
    /// False while the week is still open; its numbers will grow.
    pub complete: bool,
    pub total: Cost,
    pub by_step: Vec<GroupCost>,
    pub by_harness: Vec<GroupCost>,
    pub by_task: Vec<GroupCost>,
    pub turn_time: Vec<TurnBand>,
}

#[derive(Debug, Serialize)]
pub struct GroupCost {
    pub name: String,
    #[serde(flatten)]
    pub cost: Cost,
}

#[derive(Debug, Serialize)]
pub struct Cost {
    pub steps: usize,
    /// Steps whose provider reported no input or no output count.
    pub unmeasured_steps: usize,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub input_output_ratio: Option<f64>,
    pub input_per_step: Option<f64>,
    pub silent_hours: f64,
    pub silences: usize,
}

#[derive(Debug, Serialize)]
pub struct TurnBand {
    pub harness: String,
    /// Peak input tokens during the turn; "unknown" when the provider gave none.
    pub band: &'static str,
    pub turns: usize,
    pub median_minutes: f64,
    pub p90_minutes: f64,
}

/// One captured input reduced to what the report measures.
#[derive(Debug)]
struct Step {
    at: i64,
    skill: Option<String>,
    harness: String,
    task: Option<String>,
    tokens: Option<(i64, i64)>,
    turns: Vec<Turn>,
    silences: Vec<i64>,
}

#[derive(Debug)]
struct Turn {
    peak_input: Option<i64>,
    started_at: i64,
    completed_at: i64,
}

#[derive(Default)]
struct Tally {
    steps: usize,
    unmeasured_steps: usize,
    input: i64,
    output: i64,
    silent_seconds: i64,
    silences: usize,
}

impl Tally {
    fn add(&mut self, step: &Step) {
        self.steps += 1;
        match step.tokens {
            Some((input, output)) => {
                self.input += input;
                self.output += output;
            }
            None => self.unmeasured_steps += 1,
        }
        self.silent_seconds += step.silences.iter().sum::<i64>();
        self.silences += step.silences.len();
    }

    fn cost(&self) -> Cost {
        let measured = self.steps - self.unmeasured_steps;
        Cost {
            steps: self.steps,
            unmeasured_steps: self.unmeasured_steps,
            input_tokens: self.input,
            output_tokens: self.output,
            input_output_ratio: (self.output > 0).then(|| self.input as f64 / self.output as f64),
            input_per_step: (measured > 0).then(|| self.input as f64 / measured as f64),
            silent_hours: self.silent_seconds as f64 / 3600.0,
            silences: self.silences,
        }
    }
}

pub(crate) fn run(json: bool, filter: WorkFilter) -> Result<()> {
    let now = OffsetDateTime::now_utc();
    let report = build_report(&read_steps(filter)?, now.unix_timestamp());
    if json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print!("{}", report_text(&report));
    }
    if filter.wave.is_none() && filter.project.is_none() && filter.task.is_none() {
        for line in publish(&report, now) {
            eprintln!("metric observation · {line}");
        }
    }
    Ok(())
}

fn read_steps(filter: WorkFilter) -> Result<Vec<Step>> {
    let database = crate::store::database_path_from_env()?;
    if !database.exists() {
        return Ok(Vec::new());
    }
    let store = crate::store::sqlite::SqliteStore::open_execs_read_only(&database)?;
    let sessions = store.conversation_history(
        filter.wave,
        filter.project,
        filter.task,
        None,
        FIRST_WEEK_START,
        false,
    )?;
    sessions
        .iter()
        .map(|session| {
            let times = match &session.artifact_key {
                Some(input) => store.input_event_times(input)?,
                None => Vec::new(),
            };
            Ok(step(session, &times))
        })
        .collect()
}

fn step(session: &SessionHistory, event_times: &[i64]) -> Step {
    let turns: Vec<Turn> = session
        .providers
        .iter()
        .filter_map(|turn| {
            Some(Turn {
                peak_input: turn.usage.peak_input_tokens,
                started_at: turn.started_at?,
                completed_at: turn.completed_at?,
            })
        })
        .collect();
    Step {
        at: session.observed_at,
        skill: session.skill.clone(),
        harness: session.harness.clone(),
        task: session.task_identifier.clone(),
        tokens: session
            .usage
            .total_input_tokens
            .or(session.usage.input_tokens)
            .zip(session.usage.output_tokens),
        silences: silences(event_times, &turns),
        turns,
    }
}

/// Gaps over the threshold between consecutive events inside a turn that
/// completed. Waiting between turns is a person's time, not the provider's.
fn silences(event_times: &[i64], turns: &[Turn]) -> Vec<i64> {
    event_times
        .windows(2)
        .filter(|pair| pair[1] - pair[0] > SILENCE_SECONDS)
        .filter(|pair| {
            turns
                .iter()
                .any(|turn| turn.started_at <= pair[0] && pair[1] <= turn.completed_at)
        })
        .map(|pair| pair[1] - pair[0])
        .collect()
}

fn build_report(steps: &[Step], now: i64) -> ContextCostReport {
    let mut weeks = BTreeMap::<i64, Vec<&Step>>::new();
    let mut start = FIRST_WEEK_START;
    while start <= now {
        weeks.insert(start, Vec::new());
        start += WEEK_SECONDS;
    }
    for step in steps {
        let start = step.at - (step.at - FIRST_WEEK_START).rem_euclid(WEEK_SECONDS);
        if let Some(week) = weeks.get_mut(&start) {
            week.push(step);
        }
    }
    ContextCostReport {
        silence_threshold_seconds: SILENCE_SECONDS,
        weeks: weeks
            .into_iter()
            .map(|(start, steps)| week_cost(start, &steps, now))
            .collect(),
    }
}

fn week_cost(start: i64, steps: &[&Step], now: i64) -> WeekCost {
    let end = start + WEEK_SECONDS;
    let mut total = Tally::default();
    for step in steps {
        total.add(step);
    }
    WeekCost {
        start: timestamp(start),
        end: timestamp(end),
        complete: end <= now,
        total: total.cost(),
        by_step: grouped(steps, |step| step.skill.as_deref()),
        by_harness: grouped(steps, |step| Some(&step.harness)),
        by_task: grouped(steps, |step| step.task.as_deref()),
        turn_time: turn_bands(steps),
    }
}

fn timestamp(unix: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(unix).expect("report weeks stay in the calendar range")
}

/// Largest input first; steps without the attribute share a "-" row.
fn grouped<'a>(steps: &[&'a Step], key: impl Fn(&'a Step) -> Option<&'a str>) -> Vec<GroupCost> {
    let mut tallies = BTreeMap::<&str, Tally>::new();
    for step in steps {
        tallies
            .entry(key(step).unwrap_or("-"))
            .or_default()
            .add(step);
    }
    let mut groups: Vec<GroupCost> = tallies
        .into_iter()
        .map(|(name, tally)| GroupCost {
            name: name.to_string(),
            cost: tally.cost(),
        })
        .collect();
    groups.sort_by_key(|group| std::cmp::Reverse(group.cost.input_tokens));
    groups
}

/// Index into `BANDS`; turns without a peak sort after every band.
fn band(peak_input: Option<i64>) -> usize {
    peak_input.map_or(BANDS.len(), |peak| {
        BANDS
            .iter()
            .rposition(|(_, floor)| peak >= *floor)
            .unwrap_or(0)
    })
}

fn turn_bands(steps: &[&Step]) -> Vec<TurnBand> {
    let mut seconds = BTreeMap::<(&str, usize), Vec<i64>>::new();
    for step in steps {
        for turn in &step.turns {
            seconds
                .entry((&step.harness, band(turn.peak_input)))
                .or_default()
                .push(turn.completed_at - turn.started_at);
        }
    }
    seconds
        .into_iter()
        .map(|((harness, band), mut seconds)| {
            seconds.sort_unstable();
            TurnBand {
                harness: harness.to_string(),
                band: BANDS.get(band).map_or("unknown", |(label, _)| label),
                turns: seconds.len(),
                median_minutes: percentile(&seconds, 0.5) / 60.0,
                p90_minutes: percentile(&seconds, 0.9) / 60.0,
            }
        })
        .collect()
}

/// Nearest-rank percentile of a sorted, nonempty sample.
fn percentile(sorted: &[i64], quantile: f64) -> f64 {
    let rank = (quantile * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1] as f64
}

// Metric observations

/// The latest complete week, pushed to the Intelligence Wave's metrics.
fn observations(report: &ContextCostReport) -> Vec<MetricProducerObservation> {
    let Some(week) = report.weeks.iter().rev().find(|week| week.complete) else {
        return Vec::new();
    };
    // Harness rows keep their own percentiles; the Wave metric reads the worst.
    let slow = week
        .turn_time
        .iter()
        .filter(|band| band.band == SLOW_BAND)
        .map(|band| band.p90_minutes)
        .reduce(f64::max);
    let readings = [
        (
            "context-input-output-ratio",
            week.total.input_output_ratio,
            "no step reported output tokens this week",
        ),
        (
            "context-input-per-step",
            week.total.input_per_step,
            "no step reported token usage this week",
        ),
        (
            "context-slow-turn-minutes",
            slow,
            "no completed turn peaked at 200k input tokens this week",
        ),
        (
            "context-silent-hours",
            (week.total.steps > 0).then_some(week.total.silent_hours),
            "no steps were recorded this week",
        ),
    ];
    readings
        .into_iter()
        .map(|(metric_id, value, reason)| match value {
            Some(value) => MetricProducerObservation::Observed {
                wave: WAVE.to_string(),
                metric_id: metric_id.to_string(),
                instrument: INSTRUMENT.to_string(),
                value,
                source_window_start: week.start,
                source_window_end: week.end,
                complete: true,
            },
            None => MetricProducerObservation::Unavailable {
                wave: WAVE.to_string(),
                metric_id: metric_id.to_string(),
                instrument: INSTRUMENT.to_string(),
                source_as_of: week.end,
                reason: reason.to_string(),
            },
        })
        .collect()
}

/// A report still prints when its Wave metrics cannot be written.
fn publish(report: &ContextCostReport, now: OffsetDateTime) -> Vec<String> {
    let observations = observations(report);
    if observations.is_empty() {
        return Vec::new();
    }
    let published = (|| -> Result<Vec<String>> {
        let Some(repo) = crate::repository::CanonicalRepo::current()? else {
            return Ok(vec!["not in a repository; not persisted".to_string()]);
        };
        tokio::runtime::Runtime::new()?.block_on(async {
            let Some(store) = crate::store::open_existing_store().await else {
                return Ok(vec!["no local Loopflow registry; not persisted".to_string()]);
            };
            crate::ops::metrics::publish_metric_observations(
                &store,
                repo.as_path(),
                observations,
                now,
            )
            .await
        })
    })();
    published.unwrap_or_else(|error| vec![format!("not persisted: {error:#}")])
}

// Text

fn report_text(report: &ContextCostReport) -> String {
    let colors = Colors::default();
    let mut out = format!(
        "{}CONTEXT COST BY WEEK{} (silence: over {} min inside a turn)\n",
        colors.bold,
        colors.reset,
        report.silence_threshold_seconds / 60
    );
    out.push_str(&format!("{}\n", cost_header("WEEK OF")));
    for week in &report.weeks {
        out.push_str(&format!(
            "{:<22}  {}{}\n",
            week.start.date().to_string(),
            cost_columns(&week.total),
            if week.complete { "" } else { "  (open)" }
        ));
    }
    let Some(week) = report.weeks.iter().rev().find(|week| week.total.steps > 0) else {
        out.push_str("No Session inputs recorded since the baseline week.\n");
        return out;
    };
    out.push_str(&format!("\nWeek of {}\n", week.start.date()));
    for (title, groups) in [
        ("STEP", &week.by_step[..]),
        ("HARNESS", &week.by_harness[..]),
        ("TASK", &week.by_task[..week.by_task.len().min(TASK_ROWS)]),
    ] {
        out.push_str(&format!("\n{}\n", cost_header(title)));
        for group in groups {
            out.push_str(&format!(
                "{:<22}  {}\n",
                truncate(&group.name, 22),
                cost_columns(&group.cost)
            ));
        }
    }
    if week.by_task.len() > TASK_ROWS {
        out.push_str(&format!(
            "… {} more Tasks in --json\n",
            week.by_task.len() - TASK_ROWS
        ));
    }
    out.push_str(&format!(
        "\n{:<22}  {:<9}  {:>6}  {:>10}  {:>10}\n",
        "TURN TIME", "PEAK", "TURNS", "MEDIAN MIN", "P90 MIN"
    ));
    for band in &week.turn_time {
        out.push_str(&format!(
            "{:<22}  {:<9}  {:>6}  {:>10.1}  {:>10.1}\n",
            truncate(&band.harness, 22),
            band.band,
            band.turns,
            band.median_minutes,
            band.p90_minutes
        ));
    }
    out
}

fn cost_header(title: &str) -> String {
    format!(
        "{:<22}  {:>6}  {:>15}  {:>13}  {:>8}  {:>12}  {:>9}  {:>5}",
        title, "STEPS", "INPUT", "OUTPUT", "IN:OUT", "INPUT/STEP", "SILENT H", "GAPS"
    )
}

fn cost_columns(cost: &Cost) -> String {
    let tokens = |value: i64| format_int(u64::try_from(value).unwrap_or(0));
    let steps = if cost.unmeasured_steps == 0 {
        cost.steps.to_string()
    } else {
        // Some of these steps reported no usage and are outside the ratios.
        format!("{}*", cost.steps)
    };
    format!(
        "{:>6}  {:>15}  {:>13}  {:>8}  {:>12}  {:>9.1}  {:>5}",
        steps,
        tokens(cost.input_tokens),
        tokens(cost.output_tokens),
        cost.input_output_ratio
            .map(|ratio| format!("{ratio:.0}:1"))
            .unwrap_or_else(|| "-".to_string()),
        cost.input_per_step
            .map(|input| tokens(input as i64))
            .unwrap_or_else(|| "-".to_string()),
        cost.silent_hours,
        cost.silences
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    fn step_at(at: i64, harness: &str, skill: &str, tokens: Option<(i64, i64)>) -> Step {
        Step {
            at,
            skill: Some(skill.to_string()),
            harness: harness.to_string(),
            task: None,
            tokens,
            turns: Vec::new(),
            silences: Vec::new(),
        }
    }

    #[test]
    fn weeks_start_at_the_baseline_and_report_the_trend() {
        let mut later = step_at(
            FIRST_WEEK_START + 8 * DAY,
            "claude",
            "implement",
            Some((1_000, 10)),
        );
        later.task = Some("LOO-348".to_string());
        let steps = [
            step_at(
                FIRST_WEEK_START + DAY,
                "codex",
                "implement",
                Some((4_200, 10)),
            ),
            step_at(FIRST_WEEK_START + 2 * DAY, "codex", "gate", None),
            later,
        ];
        let report = build_report(&steps, FIRST_WEEK_START + 9 * DAY);

        assert_eq!(report.weeks.len(), 2);
        let baseline = &report.weeks[0];
        assert!(baseline.complete);
        assert_eq!(baseline.total.steps, 2);
        assert_eq!(baseline.total.unmeasured_steps, 1);
        assert_eq!(baseline.total.input_output_ratio, Some(420.0));
        assert_eq!(baseline.total.input_per_step, Some(4_200.0));
        assert_eq!(baseline.by_step[0].name, "implement");
        let open = &report.weeks[1];
        assert!(!open.complete);
        assert_eq!(open.total.input_output_ratio, Some(100.0));
        assert_eq!(open.by_task[0].name, "LOO-348");
        assert_eq!(open.by_harness[0].name, "claude");
    }

    #[test]
    fn turn_time_is_banded_by_peak_input_per_harness() {
        let mut step = step_at(FIRST_WEEK_START, "codex", "implement", Some((1, 1)));
        step.turns = vec![
            Turn {
                peak_input: Some(99_999),
                started_at: 0,
                completed_at: 60,
            },
            Turn {
                peak_input: Some(210_000),
                started_at: 0,
                completed_at: 600,
            },
            Turn {
                peak_input: Some(250_000),
                started_at: 0,
                completed_at: 2_880,
            },
            Turn {
                peak_input: None,
                started_at: 0,
                completed_at: 30,
            },
        ];
        let report = build_report(&[step], FIRST_WEEK_START + DAY);
        let bands = &report.weeks[0].turn_time;

        let labels: Vec<_> = bands.iter().map(|band| band.band).collect();
        assert_eq!(labels, ["<100k", "200k+", "unknown"]);
        assert_eq!(bands[1].turns, 2);
        assert_eq!(bands[1].median_minutes, 10.0);
        assert_eq!(bands[1].p90_minutes, 48.0);
    }

    #[test]
    fn silence_counts_only_long_gaps_inside_a_completed_turn() {
        // 0→700 is inside the turn; 800→2000 spans its end; 2000→2100 is short.
        let gaps = silences(
            &[0, 700, 800, 2_000, 2_100],
            &[Turn {
                peak_input: None,
                started_at: 0,
                completed_at: 900,
            }],
        );
        assert_eq!(gaps, [700]);
    }

    #[test]
    fn only_a_complete_week_becomes_a_wave_reading() {
        let mut step = step_at(FIRST_WEEK_START, "codex", "implement", Some((840, 2)));
        step.silences = vec![7_200];
        let open = build_report(std::slice::from_ref(&step), FIRST_WEEK_START + DAY);
        assert!(observations(&open).is_empty());

        let closed = build_report(&[step], FIRST_WEEK_START + 8 * DAY);
        let readings = observations(&closed);
        assert_eq!(readings.len(), 4);
        assert!(matches!(
            &readings[0],
            MetricProducerObservation::Observed { metric_id, value, .. }
                if metric_id == "context-input-output-ratio" && *value == 420.0
        ));
        // No turn reached 200k, so the slow-turn reading is absent, not zero.
        assert!(matches!(
            &readings[2],
            MetricProducerObservation::Unavailable { metric_id, .. }
                if metric_id == "context-slow-turn-minutes"
        ));
        assert!(matches!(
            &readings[3],
            MetricProducerObservation::Observed { value, .. } if *value == 2.0
        ));
    }
}
