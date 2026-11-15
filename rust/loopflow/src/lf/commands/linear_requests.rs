//! `lf history linear` — where this Machine's Linear requests went, by hour,
//! command and operation, from the request log `logs.linear_requests` turns on.

use anyhow::Result;
use time::OffsetDateTime;

use crate::lf::output::{format_int, format_time, truncate, Colors};
use crate::linear::request_log::{self, RequestOutcome, RequestSummary, Tally};

const COMMAND_WIDTH: usize = 24;
const OPERATION_WIDTH: usize = 28;

pub fn run(since: &str, json: bool) -> Result<()> {
    let home = crate::store::lf_home_dir();
    let since = super::util::parse_since(since, OffsetDateTime::now_utc())?;
    let since_ms = since.unix_timestamp() * 1000;
    let summary = request_log::summarize(since_ms, &request_log::read_since(&home, since_ms)?);
    if json {
        println!("{}", serde_json::to_string(&summary)?);
        return Ok(());
    }
    if !request_log::enabled(&home) {
        println!(
            "Linear request logging is off on this Machine. Turn it on in {}:\n\n  logs:\n    linear_requests: true\n",
            home.join("config.yaml").display()
        );
    }
    print!("{}", render(&summary));
    Ok(())
}

fn render(summary: &RequestSummary) -> String {
    let colors = Colors::default();
    let since = format_time(summary.since_ms / 1000);
    if summary.hours.is_empty() {
        return format!("No Linear requests recorded since {since}.\n");
    }
    let count = |outcome| summary.outcomes.get(&outcome).copied().unwrap_or(0);
    let mut out = format!(
        "{bold}LINEAR REQUESTS since {since}{reset}\n{} sent, {} rate-limited, {} failed; {} held back ({} paused, {} deferred)\n",
        format_int(count(RequestOutcome::Sent)),
        format_int(count(RequestOutcome::RateLimited)),
        format_int(count(RequestOutcome::Failed)),
        format_int(count(RequestOutcome::Paused) + count(RequestOutcome::Deferred)),
        format_int(count(RequestOutcome::Paused)),
        format_int(count(RequestOutcome::Deferred)),
        bold = colors.bold,
        reset = colors.reset,
    );
    for (label, allowance) in [
        ("requests", summary.requests),
        ("complexity", summary.complexity),
    ] {
        let Some(allowance) = allowance else { continue };
        let limit = allowance.limit.map_or_else(|| "?".into(), signed);
        let reset = allowance
            .reset_ms
            .map_or_else(|| "?".into(), |reset| format_time(reset / 1000));
        out.push_str(&format!(
            "Latest {label} allowance: {} of {limit} left, resets {reset}\n",
            signed(allowance.remaining),
        ));
    }
    out.push_str(&format!(
        "\n{bold}{:<14}  {:>8}  {:>8}  {:>10}{reset}\n",
        "HOUR",
        "SENT",
        "HELD",
        "COMPLEXITY",
        bold = colors.bold,
        reset = colors.reset,
    ));
    for hour in &summary.hours {
        out.push_str(&format!(
            "{:<14}  {}\n",
            format_time(hour.start_ms / 1000),
            tally(&hour.tally)
        ));
    }
    out.push_str(&format!(
        "\n{bold}{:<COMMAND_WIDTH$}  {:<OPERATION_WIDTH$}  {:>8}  {:>8}  {:>10}{reset}\n",
        "COMMAND",
        "OPERATION",
        "SENT",
        "HELD",
        "COMPLEXITY",
        bold = colors.bold,
        reset = colors.reset,
    ));
    for caller in &summary.callers {
        out.push_str(&format!(
            "{:<COMMAND_WIDTH$}  {:<OPERATION_WIDTH$}  {}\n",
            truncate(&caller.command, COMMAND_WIDTH),
            truncate(&caller.operation, OPERATION_WIDTH),
            tally(&caller.tally)
        ));
    }
    out
}

fn tally(tally: &Tally) -> String {
    format!(
        "{:>8}  {:>8}  {:>10}",
        format_int(tally.spent),
        format_int(tally.held),
        signed(tally.cost)
    )
}

/// Linear reports these as signed; a negative one reads as none.
fn signed(value: i64) -> String {
    format_int(u64::try_from(value).unwrap_or(0))
}
