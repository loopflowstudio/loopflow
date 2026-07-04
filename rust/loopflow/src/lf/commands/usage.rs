use std::time::Duration;

use anyhow::{anyhow, Result};

use crate::lf::output::Colors;
use crate::lfd::client::{authorize, blocking_client, resolve_base_url};
use crate::lfd::http::dto::{ProviderUsageDto, RepoProviderUsageDto, UsageReportDto};

const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const REPO_WIDTH: usize = 32;
const PROVIDER_WIDTH: usize = 12;
const NUM_WIDTH: usize = 14;

/// Fetch token usage from a running `lfd` and print a repo x provider table
/// with per-provider and grand-total rollups.
pub fn run() -> Result<()> {
    let report = fetch_report()?;
    print_report(&report);
    Ok(())
}

fn fetch_report() -> Result<UsageReportDto> {
    let client = blocking_client(FETCH_TIMEOUT)?;
    let url = format!("{}/v0/usage", resolve_base_url());
    let response = authorize(client.get(&url))
        .send()
        .map_err(|err| anyhow!("failed to reach lfd at {url}: {err}"))?;
    if !response.status().is_success() {
        return Err(anyhow!(
            "lfd returned status {} for {url}",
            response.status()
        ));
    }
    response
        .json::<UsageReportDto>()
        .map_err(|err| anyhow!("failed to parse usage report: {err}"))
}

fn print_report(report: &UsageReportDto) {
    let colors = Colors::default();

    if report.by_repo_provider.is_empty() {
        println!("No token usage recorded yet.");
        return;
    }

    print_repo_header(&colors);
    let (mut grand_input, mut grand_output) = (0u64, 0u64);
    for row in &report.by_repo_provider {
        print_repo_row(row);
        grand_input += row.input_tokens;
        grand_output += row.output_tokens;
    }
    println!();

    print_provider_header(&colors);
    for row in &report.by_provider {
        print_provider_row(row);
    }
    println!();

    print_total_row(&colors, grand_input, grand_output);
}

fn print_repo_header(colors: &Colors) {
    println!(
        "{bold}{repo:<repo_w$}  {provider:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {total:>num_w$}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        repo = "REPO",
        provider = "PROVIDER",
        input = "INPUT",
        output = "OUTPUT",
        total = "TOTAL",
        repo_w = REPO_WIDTH,
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
    );
}

fn print_repo_row(row: &RepoProviderUsageDto) {
    let repo = row.repo.as_deref().unwrap_or("(unattributed)");
    let total = row.input_tokens + row.output_tokens;
    println!(
        "{repo:<repo_w$}  {provider:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {total:>num_w$}",
        repo = truncate(&short_repo(repo), REPO_WIDTH),
        provider = row.provider,
        input = format_int(row.input_tokens),
        output = format_int(row.output_tokens),
        total = format_int(total),
        repo_w = REPO_WIDTH,
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
    );
}

fn print_provider_header(colors: &Colors) {
    println!(
        "{bold}{provider:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {total:>num_w$}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        provider = "PROVIDER",
        input = "INPUT",
        output = "OUTPUT",
        total = "TOTAL",
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
    );
}

fn print_provider_row(row: &ProviderUsageDto) {
    let total = row.input_tokens + row.output_tokens;
    println!(
        "{provider:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {total:>num_w$}",
        provider = row.provider,
        input = format_int(row.input_tokens),
        output = format_int(row.output_tokens),
        total = format_int(total),
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
    );
}

fn print_total_row(colors: &Colors, input: u64, output: u64) {
    println!(
        "{bold}{label:<prov_w$}  {input:>num_w$}  {output:>num_w$}  {total:>num_w$}{reset}",
        bold = colors.bold,
        reset = colors.reset,
        label = "TOTAL",
        input = format_int(input),
        output = format_int(output),
        total = format_int(input + output),
        prov_w = PROVIDER_WIDTH,
        num_w = NUM_WIDTH,
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
    use super::{format_int, short_repo, truncate};

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
}
