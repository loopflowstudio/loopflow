use anyhow::Result;

use crate::journal::open_ledger;
use crate::lf::commands::runs::{boundary_spans, own_spend};
use crate::lf::output::Colors;
use crate::lfdb::{ProviderUsage, RepoProviderUsage, TokenUsageReport};

const REPO_WIDTH: usize = 32;
const PROVIDER_WIDTH: usize = 12;
const NUM_WIDTH: usize = 14;
const COST_WIDTH: usize = 10;

/// Print a repo x provider table of token usage and cost, with per-provider
/// and grand-total rollups. Reads the local run ledger directly — the same
/// store `lf runs` and `lf trace` read, and no running `lfd` required.
///
/// `--json` emits one row per *boundary* instead: what each skill, and each
/// terminal run, actually spent. That is the grain the dashboard groups by —
/// skill, `provider:model`, repo — and it is the only form in which "tokens by
/// skill" is answerable, because a skill row's reading is cumulative and must
/// be diffed against the boundary before it.
pub fn run(json: bool, days: u32) -> Result<()> {
    if json {
        return print_spend_json(days);
    }
    let report = open_ledger()?.aggregate_token_usage()?;
    print_report(&report);
    Ok(())
}

fn print_spend_json(days: u32) -> Result<()> {
    let since = time::OffsetDateTime::now_utc().unix_timestamp() - i64::from(days) * 86_400;
    let events = open_ledger()?.list_run_events_since(since)?;
    let spend = own_spend(&boundary_spans(&events));
    println!("{}", serde_json::to_string(&spend)?);
    Ok(())
}

/// Rows recorded before the provider dimension existed, and runs whose repo
/// was never resolved, group under a NULL key.
fn or_unattributed(value: Option<&str>) -> &str {
    value.unwrap_or("(unattributed)")
}

fn print_report(report: &TokenUsageReport) {
    let colors = Colors::default();

    if report.by_repo_provider.is_empty() {
        println!("No token usage recorded yet.");
        return;
    }

    print_repo_header(&colors);
    let (mut grand_input, mut grand_output, mut grand_cache, mut grand_cost) =
        (0u64, 0u64, 0u64, 0.0);
    for row in &report.by_repo_provider {
        print_repo_row(row);
        grand_input += row.input_tokens;
        grand_output += row.output_tokens;
        grand_cache += row.cache_read_tokens;
        grand_cost += row.cost_usd;
    }
    println!();

    print_provider_header(&colors);
    for row in &report.by_provider {
        print_provider_row(row);
    }
    println!();

    print_total_row(&colors, grand_input, grand_output, grand_cache, grand_cost);
}

fn print_repo_header(colors: &Colors) {
    println!(
        "{bold}{repo:<repo_w$}  {provider:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {cache:>num_w$}  {total:>num_w$}  {cost:>cost_w$}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        repo = "REPO",
        provider = "PROVIDER",
        input = "INPUT",
        output = "OUTPUT",
        cache = "CACHE READ",
        total = "TOTAL",
        cost = "COST",
        repo_w = REPO_WIDTH,
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
        cost_w = COST_WIDTH,
    );
}

fn print_repo_row(row: &RepoProviderUsage) {
    let repo = or_unattributed(row.repo.as_deref());
    let total = row.input_tokens + row.output_tokens;
    println!(
        "{repo:<repo_w$}  {provider:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {cache:>num_w$}  {total:>num_w$}  {cost:>cost_w$}",
        repo = truncate(&short_repo(repo), REPO_WIDTH),
        provider = or_unattributed(row.provider.as_deref()),
        input = format_int(row.input_tokens),
        output = format_int(row.output_tokens),
        cache = format_int(row.cache_read_tokens),
        total = format_int(total),
        cost = format_cost(row.cost_usd),
        repo_w = REPO_WIDTH,
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
        cost_w = COST_WIDTH,
    );
}

fn print_provider_header(colors: &Colors) {
    println!(
        "{bold}{provider:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {cache:>num_w$}  {total:>num_w$}  {cost:>cost_w$}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        provider = "PROVIDER",
        input = "INPUT",
        output = "OUTPUT",
        cache = "CACHE READ",
        total = "TOTAL",
        cost = "COST",
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
        cost_w = COST_WIDTH,
    );
}

fn print_provider_row(row: &ProviderUsage) {
    let total = row.input_tokens + row.output_tokens;
    println!(
        "{provider:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {cache:>num_w$}  {total:>num_w$}  {cost:>cost_w$}",
        provider = or_unattributed(row.provider.as_deref()),
        input = format_int(row.input_tokens),
        output = format_int(row.output_tokens),
        cache = format_int(row.cache_read_tokens),
        total = format_int(total),
        cost = format_cost(row.cost_usd),
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
        cost_w = COST_WIDTH,
    );
}

fn print_total_row(colors: &Colors, input: u64, output: u64, cache: u64, cost: f64) {
    println!(
        "{bold}{label:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {cache:>num_w$}  {total:>num_w$}  {cost:>cost_w$}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        label = "TOTAL",
        input = format_int(input),
        output = format_int(output),
        cache = format_int(cache),
        total = format_int(input + output),
        cost = format_cost(cost),
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
        cost_w = COST_WIDTH,
    );
}

/// Show just the final path component when the repo is an absolute path, so
/// the table stays scannable; keep short names or slugs verbatim.
fn short_repo(repo: &str) -> String {
    repo.rsplit('/')
        .find(|segment| !segment.is_empty())
        .unwrap_or(repo)
        .to_string()
}

fn truncate(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_string();
    }
    let head: String = value.chars().take(width.saturating_sub(1)).collect();
    format!("{head}\u{2026}")
}

fn format_cost(value: f64) -> String {
    format!("${value:.2}")
}

fn format_int(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (idx, ch) in digits.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::{format_cost, format_int, or_unattributed, short_repo, truncate};

    #[test]
    fn format_int_groups_thousands() {
        assert_eq!(format_int(0), "0");
        assert_eq!(format_int(999), "999");
        assert_eq!(format_int(1_000), "1,000");
        assert_eq!(format_int(1_234_567), "1,234,567");
    }

    #[test]
    fn short_repo_takes_last_path_segment() {
        assert_eq!(short_repo("/Users/jack/src/loopflow"), "loopflow");
        assert_eq!(short_repo("loopflow"), "loopflow");
        assert_eq!(short_repo("/Users/jack/src/cadenza/"), "cadenza");
    }

    #[test]
    fn truncate_adds_ellipsis_past_width() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd\u{2026}");
    }

    #[test]
    fn format_cost_always_shows_cents() {
        assert_eq!(format_cost(0.0), "$0.00");
        assert_eq!(format_cost(61.851), "$61.85");
    }

    #[test]
    fn null_repo_and_provider_read_as_unattributed() {
        assert_eq!(or_unattributed(None), "(unattributed)");
        assert_eq!(or_unattributed(Some("claude")), "claude");
    }
}
